// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//#![deny(warnings)]
#![feature(
    async_await,
    await_macro,
    futures_api,
    pin,
    arbitrary_self_types
)]

use failure::{Error, ResultExt};
use fidl::endpoints::{RequestStream, ServerEnd, ServiceMarker};
use fidl_fuchsia_telephony_ril::{RadioInterfaceLayerMarker, RadioInterfaceLayerRequest, RadioInterfaceLayerRequestStream};
use fuchsia_app::server::ServicesServer;
use fuchsia_async as fasync;
use fuchsia_syslog::{self as syslog, macros::*};
use fuchsia_zircon as zx;
use futures::{TryFutureExt, TryStreamExt};
use qmi_protocol::QmiResult;

use parking_lot::{Mutex, RwLock};
use std::sync::Arc;

use crate::client::QmiClient;
use crate::transport::QmiTransport;
use crate::errors::QmuxError;
use qmi_protocol::CTL::*;
use qmi_protocol::*;

mod client;
mod errors;
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

    pub async fn create_client(&self) -> Result<QmiClient, Error> {
        fx_log_info!("Client connecting...");
        if let Some(ref inner) = self.inner {
            let transport_inner = inner.clone();
            let client = QmiClient::new(transport_inner);
            Ok(client) // Arc::new(RwLock::new(client)))
        } else {
            Err(QmuxError::NoClient.into())
        }
    }
}

type ClientPtr = Arc<Mutex<Option<QmiClient>>>;

struct FrilService;
impl FrilService {
    pub fn spawn(modem: QmiModemPtr, chan: fasync::Channel) {
        let client = Arc::new(Mutex::new(None));
        let server = RadioInterfaceLayerRequestStream::from_channel(chan)
            .try_for_each(move |req| Self::handle_request(modem.clone(), client.clone(), req))
            .unwrap_or_else(|e| fx_log_err!("Error running {:?}", e));
        fasync::spawn(server);
    }

    async fn handle_request(
        modem: QmiModemPtr, mut client: ClientPtr, request: RadioInterfaceLayerRequest,
    ) -> Result<(), fidl::Error> {
        match request {
            RadioInterfaceLayerRequest::ConnectTransport { channel, responder } => {
                let mut lock = modem.lock();
                let status = lock.connect_transport(channel);
                fx_log_info!("Connecting the service to the transport driver: {}", status);
                responder.send(status)
            }
            RadioInterfaceLayerRequest::GetDeviceIdentity { responder } => {
                let mut client_lock = client.lock();

                if client_lock.is_none() {
                    fx_log_info!("Requested client connect.");
                    eprintln!("strong count: {}", Arc::strong_count(&client));
                    let modem_lock = modem.lock();

                    // TODO RIL Error type
                    //if !lock.connected() {
                    //    return responder.send(false);
                    //}
                    //let client = client.lock();
                    let alloced_client = await!(modem_lock.create_client()).unwrap();
                    *client_lock = Some(alloced_client);
                }

                if let Some(ref client) = *client_lock {
                    fx_log_info!("send serial request!");
                    //let resp: Result<QmiResult<DMS::GetDeviceSerialNumbersResp>, QmuxError>
                    //    = await!(client.send_msg(DMS::GetDeviceSerialNumbersReq::new()));
                    //fx_log_info!("Device serial numbers resp: {:?}", resp);
                    //let resp: Result<QmiResult<CTL::GetVersionInfoResp>, QmuxError>
                    //    = await!(client.send_msg(CTL::GetVersionInfoReq::new()));
                    //fx_log_info!("Device version info {:?}", resp);
                }

            //    if client.is_ok() {
            //        QmiClientService::spawn(channel, client.unwrap());
            //        responder.send(true)
            //    } else {
                    responder.send(3)
            //    }
            }
        }
    }
}

fn main() -> Result<(), Error> {
    syslog::init_with_tags(&["ril-qmi"]).expect("Can't init logger");
    fx_log_info!("Starting ril-qmi...");

    let mut executor = fasync::Executor::new().context("Error creating executor")?;

    let modem = Arc::new(Mutex::new(QmiModem::new()));

    let server = ServicesServer::new()
        .add_service((RadioInterfaceLayerMarker::NAME, move |chan: fasync::Channel| {
            fx_log_info!("New client connecting to the Fuchsia RIL");
            FrilService::spawn(modem.clone(), chan)
        })).start()
        .context("Error starting QMI modem service")?;

    executor.run_singlethreaded(server)
}
