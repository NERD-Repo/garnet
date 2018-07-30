// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use known_ess_store::{KnownEssStore, KnownEss};
use failure;
use fidl::endpoints2::create_endpoints;
use fidl_sme;
use future_util::retry_until;
use futures::{prelude::*, channel::oneshot, channel::mpsc, stream};
use state_machine::{self, IntoStateExt, Never};
use std::sync::Arc;
use zx::prelude::*;

const AUTO_CONNECT_RETRY_SECONDS: u64 = 10;
const AUTO_CONNECT_SCAN_TIMEOUT_SECONDS: u8 = 20;

#[derive(Clone)]
pub struct Client {
    req_sender: mpsc::UnboundedSender<ManualRequest>,
}

impl Client {
    pub fn connect(&self, request: ConnectRequest) -> Result<(), failure::Error> {
        self.req_sender.unbounded_send(ManualRequest::Connect(request))
            .map_err(|_| format_err!("Station does not exist anymore"))
    }
}

pub struct ConnectRequest {
    pub ssid: Vec<u8>,
    pub password: Vec<u8>,
    pub responder: oneshot::Sender<fidl_sme::ConnectResultCode>,
}

enum ManualRequest {
    Connect(ConnectRequest),
}

pub fn new_client(iface_id: u16,
                  sme: fidl_sme::ClientSmeProxy,
                  ess_store: Arc<KnownEssStore>)
    -> (Client, impl Future<Output = ()>)
{
    let (req_sender, req_receiver) = mpsc::unbounded();
    let sme_event_stream = sme.take_event_stream();
    let services = Services {
        sme,
        ess_store: Arc::clone(&ess_store)
    };
    let state_machine = auto_connect_state(services, req_receiver.into_future()).into_future()
        // The future will never complete without encountering an error.
        // Set the success type to ().
        .map_ok(Never::never_into::<()>)
        .unwrap_or_else(move |e| eprintln!("wlancfg: Client station state machine \
                    for iface {} terminated with an error: {}", iface_id, e));
    let removal_watcher = sme_event_stream.map(|res| res.map(|_| ())).try_collect::<()>()
        .map_ok(move |_| println!("wlancfg: Client station removed (iface {})", iface_id))
        .unwrap_or_else(move |e|
            println!("wlancfg: Removing client station (iface {}) because of an error: {}",
                iface_id, e));
    let fut = select2(state_machine, removal_watcher);
    let client = Client { req_sender };
    (client, fut)
}

async fn select2<A: Future, B: Future>(a: A, b: B) {
    pin_mut!(a, b);
    select! {
        a => (),
        b => (),
    }
}

type State = state_machine::State<failure::Error>;
type NextReqFut = stream::StreamFuture<mpsc::UnboundedReceiver<ManualRequest>>;

#[derive(Clone)]
struct Services {
    sme: fidl_sme::ClientSmeProxy,
    ess_store: Arc<KnownEssStore>,
}

fn auto_connect_state(services: Services, next_req: NextReqFut) -> State {
    auto_connect_state_inner(services, next_req).into_state()
}

async fn auto_connect_state_inner(services: Services, mut next_req: NextReqFut)
    -> Result<State, failure::Error>
{
    println!("wlancfg: Starting auto-connect loop");
    let conn = auto_connect(services.clone());
    pin_mut!(conn);

    select! {
        conn => {
            let _ssid = conn?;
            Ok(connected_state(services, next_req))
        },
        next_req => {
            let (req, req_stream) = next_req;
            handle_manual_request(services, req, req_stream)
        },
    }
}

fn handle_manual_request(services: Services,
                         req: Option<ManualRequest>,
                         req_stream: mpsc::UnboundedReceiver<ManualRequest>)
    -> Result<State, failure::Error>
{
    match req {
        Some(ManualRequest::Connect(req)) => {
            Ok(manual_connect_state(services, req_stream.into_future(), req))
        },
        None => bail!("The stream of user requests ended unexpectedly")
    }
}

fn auto_connect(services: Services)
    -> impl Future<Output = Result<Vec<u8>, failure::Error>>
{
    retry_until(AUTO_CONNECT_RETRY_SECONDS.seconds(),
        move || attempt_auto_connect(services.clone()))
}

async fn attempt_auto_connect(services: Services) -> Result<Option<Vec<u8>>, failure::Error> {
    let results = await!(fetch_scan_results(start_scan_txn(&services.sme)?))?;
    let known_networks = {
        let services = services.clone();
        results.into_iter()
            .filter_map(move |ess| {
                services.ess_store.lookup(&ess.best_bss.ssid)
                    .map(|known_ess| (ess.best_bss.ssid, known_ess))
            })
    };

    for (ssid, known_ess) in known_networks {
        let ssid_str = String::from_utf8_lossy(&ssid);
        println!("wlancfg: Auto-connecting to '{}'", ssid_str);
        let connect_txn = start_connect_txn(&services.sme, &ssid, &known_ess.password)?;
        let r = await!(wait_until_connected(connect_txn))?;
        match r {
            fidl_sme::ConnectResultCode::Success => {
                println!("wlancfg: Auto-connected to '{}'", ssid_str);
                return Ok(Some(ssid));
            },
            other => {
                println!("wlancfg: Failed to auto-connect to '{}': {:?}", ssid_str, other);
            }
        }
    }
    Ok(None)
}

fn manual_connect_state(
    services: Services, next_req: NextReqFut, req: ConnectRequest,
) -> State {
    manual_connect_state_inner(services, next_req, req).into_state()
}

async fn manual_connect_state_inner(
    services: Services, mut next_req: NextReqFut, req: ConnectRequest
) -> Result<State, failure::Error> {
    println!("wlancfg: Connecting to '{}' because of a manual request from the user",
        String::from_utf8_lossy(&req.ssid));

    services.ess_store.store(req.ssid.clone(), KnownEss {
        password: req.password.clone()
    }).unwrap_or_else(|e| eprintln!("wlancfg: Failed to store network password: {}", e));

    let connect_txn = start_connect_txn(&services.sme, &req.ssid, &req.password)?;
    let connected = wait_until_connected(connect_txn);
    pin_mut!(connected);
    select! {
        connected => {
            let error_code = connected?;
            let _ = req.responder.send(error_code);
            match error_code {
                fidl_sme::ConnectResultCode::Success => {
                    println!("wlancfg: Successfully connected to '{}'",
                             String::from_utf8_lossy(&req.ssid));
                    Ok(connected_state(services, next_req))
                },
                other => {
                    println!("wlancfg: Failed to connect to '{}': {:?}",
                             String::from_utf8_lossy(&req.ssid), other);
                    Ok(auto_connect_state(services, next_req))
                }
            }
        },
        next_req => {
            let (new_req, req_stream) = next_req;
            let _ = req.responder.send(fidl_sme::ConnectResultCode::Canceled);
            handle_manual_request(services, new_req, req_stream)
        },
    }
}

fn connected_state(services: Services, next_req: NextReqFut) -> State {
    // TODO(gbonik): monitor connection status and jump back to auto-connect state when disconnected
    next_req
        .map(|(req, req_stream)| {
            handle_manual_request(services, req, req_stream)
        }).into_state()
}

fn start_scan_txn(sme: &fidl_sme::ClientSmeProxy)
    -> Result<fidl_sme::ScanTransactionProxy, failure::Error>
{
    let (scan_txn, remote) = create_endpoints()?;
    let mut req = fidl_sme::ScanRequest {
        timeout: AUTO_CONNECT_SCAN_TIMEOUT_SECONDS,
    };
    sme.scan(&mut req, remote)?;
    Ok(scan_txn)
}

fn start_connect_txn(sme: &fidl_sme::ClientSmeProxy, ssid: &[u8], password: &[u8])
    -> Result<fidl_sme::ConnectTransactionProxy, failure::Error>
{
    let (connect_txn, remote) = create_endpoints()?;
    let mut req = fidl_sme::ConnectRequest { ssid: ssid.to_vec(), password: password.to_vec() };
    sme.connect(&mut req, Some(remote))?;
    Ok(connect_txn)
}

async fn wait_until_connected(txn: fidl_sme::ConnectTransactionProxy)
    -> Result<fidl_sme::ConnectResultCode, failure::Error>
{
    let mut stream = txn.take_event_stream();
    if let Some(e) = await!(stream.try_next())? {
        let fidl_sme::ConnectTransactionEvent::OnFinished{ code } = e;
        Ok(code)
    } else {
        Err(format_err!("Server closed the ConnectTransaction channel before sending a response"))
    }
}

async fn fetch_scan_results(txn: fidl_sme::ScanTransactionProxy)
    -> Result<Vec<fidl_sme::EssInfo>, failure::Error>
{
    let mut event_stream = txn.take_event_stream();
    let mut all_aps = vec![];

    while let Some(event) = await!(event_stream.try_next())? {
        match event {
            fidl_sme::ScanTransactionEvent::OnResult { aps } => {
                all_aps.extend(aps);
            },
            fidl_sme::ScanTransactionEvent::OnFinished {} => {},
            fidl_sme::ScanTransactionEvent::OnError { error } => {
                eprintln!("wlancfg: Scanning failed with error: {:?}", error);
            }
        }
    }

    Ok(all_aps)
}

#[cfg(test)]
mod tests {
    use super::*;
    use async;
    use fidl::endpoints2::RequestStream;
    use fidl_sme::{ClientSmeRequest, ClientSmeRequestStream};
    use futures::stream::StreamFuture;
    use std::path::Path;
    use tempdir;

    #[test]
    fn auto_connect_to_known_ess() {
        let mut exec = async::Executor::new().expect("failed to create an executor");
        let temp_dir = tempdir::TempDir::new("client_test").expect("failed to create temp dir");
        let ess_store = create_ess_store(temp_dir.path());
        let (_client, mut fut, sme_server) = create_client(Arc::clone(&ess_store));
        let mut next_sme_req = sme_server.next();

        // Expect the state machine to initiate the scan, then send results back
        assert_eq!(Ok(Async::Pending), exec.run_until_stalled(&mut fut));
        send_scan_results(&mut exec, &mut next_sme_req, &[&b"foo"[..], &b"bar"[..]]);

        // None of the returned ssids are known though, so expect the state machine to simply sleep
        assert_eq!(Ok(Async::Pending), exec.run_until_stalled(&mut fut));
        assert!(poll_sme_req(&mut exec, &mut next_sme_req).is_pending());

        // Now save a known ESS and "wait" for the next try
        ess_store.store(b"bar".to_vec(), KnownEss { password: b"qwerty".to_vec() })
            .expect("failed to store a network password");
        assert!(exec.wake_next_timer().is_some());
        assert_eq!(Ok(Async::Pending), exec.run_until_stalled(&mut fut));

        // Expect another scan request to the SME and send results
        send_scan_results(&mut exec, &mut next_sme_req, &[&b"foo"[..], &b"bar"[..]]);

        // Let the state machine process the results
        assert_eq!(Ok(Async::Pending), exec.run_until_stalled(&mut fut));

        // Expect a "connect" request to the SME and reply to it
        send_connect_result(&mut exec, &mut next_sme_req, b"bar", b"qwerty",
                            fidl_sme::ConnectResultCode::Success);

        // Let the state machine absorb the connect ack
        assert_eq!(Ok(Async::Pending), exec.run_until_stalled(&mut fut));

        // We should be in the 'connected' state now, with no further requests to the SME
        // or pending timers
        assert!(poll_sme_req(&mut exec, &mut next_sme_req).is_pending());
        assert_eq!(None, exec.wake_next_timer());
    }

    fn poll_sme_req(exec: &mut async::Executor,
                    next_sme_req: &mut StreamFuture<ClientSmeRequestStream>)
        -> Async<ClientSmeRequest>
    {
        let a = exec.run_until_stalled(next_sme_req).unwrap_or_else(|(e, _stream)| {
            panic!("error polling SME request stream: {:?}", e)
        });
        a.map(|(req, stream)| {
            *next_sme_req = stream.next();
            req.expect("did not expect the SME request stream to end")
        })
    }

    fn send_scan_results(exec: &mut async::Executor,
                         next_sme_req: &mut StreamFuture<ClientSmeRequestStream>,
                         ssids: &[&[u8]]) {
        let txn = match poll_sme_req(exec, next_sme_req) {
            Async::Ready(ClientSmeRequest::Scan { txn, .. }) => txn,
            Async::Pending => panic!("expected a request to be available"),
            _ => panic!("expected a Scan request"),
        };
        let txn = txn.into_stream().expect("failed to create a scan txn stream").control_handle();
        let mut results = Vec::new();
        for ssid in ssids {
            results.push(fidl_sme::EssInfo {
                best_bss: fidl_sme::BssInfo {
                    bssid: [0, 1, 2, 3, 4, 5],
                    ssid: ssid.to_vec(),
                    rx_dbm: -30,
                    channel: 1,
                    protected: true,
                    compatible: true,
                }
            });
        }
        txn.send_on_result(&mut results.iter_mut()).expect("failed to send scan results");
        txn.send_on_finished().expect("failed to send OnFinished to ScanTxn");
    }

    fn send_connect_result(exec: &mut async::Executor,
                           next_sme_req: &mut StreamFuture<ClientSmeRequestStream>,
                           expected_ssid: &[u8],
                           expected_password: &[u8],
                           code: fidl_sme::ConnectResultCode) {
        let txn = match poll_sme_req(exec, next_sme_req) {
            Async::Ready(ClientSmeRequest::Connect { req, txn, .. }) => {
                assert_eq!(expected_ssid, &req.ssid[..]);
                assert_eq!(expected_password, &req.password[..]);
                txn.expect("expected a Connect transaction channel")
            },
            _ => panic!("expected a Connect request"),
        };
        let txn = txn.into_stream().expect("failed to create a connect txn stream").control_handle();
        txn.send_on_finished(code).expect("failed to send OnFinished to ConnectTxn");
    }

    fn create_ess_store(path: &Path) -> Arc<KnownEssStore> {
        Arc::new(KnownEssStore::new_with_paths(path.join("store.json"), path.join("store.json.tmp"))
            .expect("failed to create an KnownEssStore"))
    }

    fn create_client(ess_store: Arc<KnownEssStore>)
        -> (Client, impl Future<Item = (), Error = Never>, ClientSmeRequestStream)
    {
        let (proxy, server) = create_endpoints::<fidl_sme::ClientSmeMarker>()
            .expect("failed to create an sme channel");
        let (client, fut) = new_client(0, proxy, Arc::clone(&ess_store));
        let server = server.into_stream().expect("failed to create a request stream");
        (client, fut, server)
    }

}
