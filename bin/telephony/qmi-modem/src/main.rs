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
#![feature(try_from)]

use failure::{Error, ResultExt};
use fidl::endpoints2::RequestStream;
use fidl::endpoints2::ServerEnd;
use fidl::endpoints2::ServiceMarker;
use fidl_fuchsia_telephony_qmi::{DeviceManagementRequest, DeviceManagementRequestStream};
use fidl_fuchsia_telephony_qmi::{QmiClientMarker, QmiClientRequest, QmiClientRequestStream};
use fidl_fuchsia_telephony_qmi::{QmiModemMarker, QmiModemRequest, QmiModemRequestStream};
use fuchsia_app::server::ServicesServer;
use fuchsia_async as fasync;
use fuchsia_syslog::{self as syslog, macros::*};
use fuchsia_zircon as zx;
use futures::{future, TryFutureExt, TryStreamExt};
use std::convert::TryInto;
use std::pin::PinMut;

use parking_lot::{Mutex, RwLock};
use std::sync::Arc;

type QmiModemPtr = Arc<Mutex<QmiModem>>;
type QmiClientPtr = Arc<RwLock<QmiClient>>;

// Many Qualcomm modems have an limit of 5 outstanding requests by default
pub const MAX_CONCURRENT: usize = 4;

pub struct QmiClient;
impl QmiClient {
    pub fn new() -> Self {
        QmiClient {
        }
    }
}

pub struct QmiModem {
    transport_channel: Option<fasync::Channel>,
}

impl QmiModem {
    pub fn new() -> Self {
        QmiModem {
            transport_channel: None,
        }
    }

    pub fn connected(&self) -> bool {
        self.transport_channel.is_some()
    }

    pub fn set_transport(&mut self, chan: zx::Channel) -> bool {
        if self.transport_channel.is_none() {
            return false;
        }
        match fasync::Channel::from_channel(chan) {
            Ok(chan) => {
                self.transport_channel = Some(chan);
                true
            }
            Err(_) => {
                fx_log_err!("Failed to convert a zircon channel to a fasync one");
                false
            }
        }
    }

    pub async fn create_client(&self) -> Result<QmiClientPtr, Error> {
        // do all the client work
        let client = QmiClient::new();
        //let resp = await!(client.send_raw_msg(&[0x01,
        //                      0x0F, 0x00,  // length
        //                      0x00,         // control flag
        //                      0x00,         // service type
        //                      0x00,         // client id
        //                      // SDU below
        //                      0x00, // control flag
        //                      0x00, // tx id
        //                      0x20, 0x00, // message id
        //                      0x04, 0x00, // Length
        //                      0x01, // type
        //                      0x01, 0x00, // length
        //                      0x48 // value
        //]));
        Ok(Arc::new(RwLock::new(client)))
    }
}

// TODO create_service!(DataManagementService,

use fidl_fuchsia_telephony_qmi::DeviceManagementMarker;
struct DataManagementService;
impl DataManagementService {
    pub fn spawn(server_end: ServerEnd<DeviceManagementMarker>, client: QmiClientPtr) {
        if let Ok(request_stream) = server_end.into_stream() {
            fasync::spawn(
                request_stream
                    .try_for_each(move |req| Self::handle_request(client.clone(), req))
                    .unwrap_or_else(|e| fx_log_err!("Error running {:?}", e)),
            );
        }
    }

    async fn handle_request(
        client: QmiClientPtr, request: DeviceManagementRequest,
    ) -> Result<(), fidl::Error> {
        match request {
            DeviceManagementRequest::SetEventReport {
                power_state,
                battery_lvl_lower_limit,
                battery_lvl_upper_limit,
                pin_state,
                activation_state,
                operator_mode_state,
                uim_state,
                responder,
            } => {
                Ok(())
            }
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
                DataManagementService::spawn(service, client.clone());
                responder.send(true)
            }
        }
    }
}

struct QmiModemService;
impl QmiModemService {
    pub fn spawn(modem: QmiModemPtr, chan: fasync::Channel) {
        let server = QmiModemRequestStream::from_channel(chan)
            .try_for_each_concurrent(MAX_CONCURRENT, move |req| {
                Self::handle_request(modem.clone(), req)
            }).unwrap_or_else(|e| fx_log_err!("Error running {:?}", e));
        fasync::spawn(server);
    }

    async fn handle_request(
        modem: QmiModemPtr, request: QmiModemRequest,
    ) -> Result<(), fidl::Error> {
        match request {
            QmiModemRequest::ConnectTransport { channel, responder } => {
                responder.send(modem.lock().set_transport(channel))
            }
            QmiModemRequest::ConnectClient { channel, responder } => {
                if !modem.lock().connected() {
                    return responder.send(false);
                }
                let m = modem.lock();
                let client = await!(m.create_client()).unwrap();
                QmiClientService::spawn(channel, client);
                responder.send(true)
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
