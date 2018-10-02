// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#![deny(warnings)]
#![feature(
    async_await,
    await_macro,
    futures_api,
    pin,
    arbitrary_self_types
)]

use failure::{format_err, Error, ResultExt};
use fidl::endpoints::RequestStream;
use fidl::endpoints::ServerEnd;
use fidl::endpoints::ServiceMarker;
use fidl_fuchsia_telephony_qmi::{QmiClientMarker, QmiClientRequest};
use fidl_fuchsia_telephony_qmi::{QmiModemMarker, QmiModemRequest, QmiModemRequestStream};
use fuchsia_app::server::ServicesServer;
use fuchsia_async as fasync;
use fuchsia_syslog::{self as syslog, macros::*};
use fuchsia_zircon as zx;
use futures::{TryFutureExt, TryStreamExt};
use qmi;
use std::io::Cursor;

use parking_lot::{Mutex, RwLock};
use std::sync::Arc;

use crate::client::QmiClient;
use crate::transport::QmiTransport;

mod client;
mod service;
mod transport;

type QmiModemPtr = Arc<Mutex<QmiModem>>;
type QmiClientPtr = Arc<RwLock<QmiClient>>;

pub struct QmiModem {
    inner: Option<Arc<QmiTransport>>,
}

impl QmiModem {
    pub fn new() -> Self {
        QmiModem { inner: None }
    }

    pub fn connected(&self) -> bool {
        // TODO add aditional logic for checking transport_channel open
        self.inner.is_some()
    }

    pub fn connect_transport(&mut self, chan: zx::Channel) -> bool {
        fx_log_info!("Connecting the transport");
        if self.connected() {
            fx_log_err!("Attempted to connect more than one transport");
            return false;
        }
        match fasync::Channel::from_channel(chan) {
            Ok(chan) => {
                if chan.is_closed() {
                    fx_log_err!("The transport channel is not open");
                    return false;
                }
                self.inner = Some(Arc::new(QmiTransport::new(chan)));
                true
            }
            Err(_) => {
                fx_log_err!("Failed to convert a zircon channel to a fasync one");
                false
            }
        }
    }

    pub async fn create_client(&self) -> Result<QmiClientPtr, Error> {
        fx_log_info!("Client connecting...");
        if let Some(ref inner) = self.inner {
            let transport_inner = inner.clone();
            let client = QmiClient::new(transport_inner);
            let resp = await!(client.send_raw_msg(
                0x00,
                0x00,
                &[
                    0x01, 0x0F, 0x00, // length
                    0x00, // control flag
                    0x00, // service type
                    0x00, // client id
                    // SDU below
                    0x00, // control flag
                    0x00, // tx id
                    0x20, 0x00, // message id
                    0x04, 0x00, // Length
                    0x01, // type
                    0x01, 0x00, // length
                    0x48  // value
                ]
            ))?;
            let mut buf = Cursor::new(resp.bytes());
            let (resp, _) = qmi::parse_set_instance_id_resp(buf);
            fx_log_info!("Instance Id Response: {:?}", resp);

            let resp = await!(client.send_raw_msg(
                0x00,
                0x00,
                &[
                    0x01, 0x0F, 0x00, // length
                    0x00, // control flag
                    0x00, // service type
                    0x00, // client id
                    // SDU below
                    0x00, // control flag
                    0x00, // tx id
                    0x22, 0x00, // message id
                    0x04, 0x00, // Length
                    0x01, // type
                    0x01, 0x00, // length
                    0x02  // value
                ]
            ))?;
            buf = Cursor::new(resp.bytes());
            let (resp, _) = qmi::parse_get_client_id(buf);
            fx_log_info!("Client Id Allocation: {:?}", resp);

            // TODO set the ID here
            Ok(Arc::new(RwLock::new(client)))
        } else {
            Err(format_err!("no client connected!"))
        }
    }
}

struct QmiClientService;
impl QmiClientService {
    pub fn spawn(server_end: ServerEnd<QmiClientMarker>, client: QmiClientPtr) {
        if let Ok(request_stream) = server_end.into_stream() {
            fasync::spawn(
                request_stream
                    .try_for_each(move |req| Self::handle_request(client.clone(), req))
                    .unwrap_or_else(|e| fx_log_err!("Error running {:?}", e)),
            );
        }
    }

    async fn handle_request(
        client: QmiClientPtr, request: QmiClientRequest,
    ) -> Result<(), fidl::Error> {
        match request {
            QmiClientRequest::RequestDataManagementService { service, responder } => {
                service::DataManagementService::spawn(service, client.clone());
                responder.send(true)
            }
        }
    }
}

struct QmiModemService;
impl QmiModemService {
    pub fn spawn(modem: QmiModemPtr, chan: fasync::Channel) {
        let server = QmiModemRequestStream::from_channel(chan)
            .try_for_each(move |req| Self::handle_request(modem.clone(), req))
            .unwrap_or_else(|e| fx_log_err!("Error running {:?}", e));
        fasync::spawn(server);
    }

    async fn handle_request(
        modem: QmiModemPtr, request: QmiModemRequest,
    ) -> Result<(), fidl::Error> {
        match request {
            QmiModemRequest::ConnectTransport { channel, responder } => {
                let mut lock = modem.lock();
                let status = lock.connect_transport(channel);
                fx_log_info!("Connecting the service to the transport driver: {}", status);
                responder.send(status)
            }
            QmiModemRequest::ConnectClient { channel, responder } => {
                fx_log_info!("Requested client connect.");
                let lock = modem.lock();
                if !lock.connected() {
                    return responder.send(false);
                }
                let client = await!(lock.create_client());
                if client.is_ok() {
                    QmiClientService::spawn(channel, client.unwrap());
                    responder.send(true)
                } else {
                    responder.send(false)
                }
            }
        }
    }
}

fn main() -> Result<(), Error> {
    syslog::init_with_tags(&["qmi-modem"]).expect("Can't init logger");
    fx_log_info!("Starting qmi-modem...");

    let mut executor = fasync::Executor::new().context("Error creating executor")?;

    let modem = Arc::new(Mutex::new(QmiModem::new()));

    let server = ServicesServer::new()
        .add_service((QmiModemMarker::NAME, move |chan: fasync::Channel| {
            fx_log_info!("client connecting to QMI modem");
            QmiModemService::spawn(modem.clone(), chan)
        })).start()
        .context("Error starting QMI modem service")?;

    executor.run_singlethreaded(server)
}
