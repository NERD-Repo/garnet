// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//#![deny(warnings)]
#![feature(async_await, await_macro, futures_api, pin, arbitrary_self_types)]
#![feature(try_from)]

use fuchsia_syslog::{self as syslog, macros::*};
use failure::{ResultExt, Error};
use fuchsia_app::server::ServicesServer;
use fidl_fuchsia_telephony_qmi::{QmiModemMarker, QmiModemRequest, QmiModemRequestStream};
use fidl::endpoints2::RequestStream;
use futures::{future, TryStreamExt, TryFutureExt};
use fuchsia_async as fasync;
use fidl::endpoints2::ServiceMarker;
use std::pin::PinMut;
use std::convert::TryInto;

// mod service;

//#[derive(QmiMsg)]
//SetInstanceId {
//    msg_id: 0x0020,
//    req: tlv!(0x01, u8, instance),
//    resp: tlv!(0x01, u16, id),
//}
//
//let msg = SetInstanceId::msg(3u8);
//let id = SetInstanceId::parse([0x01, 9u16])
//
//use qmi_protocol::QmiClient;
use qmi_protocol::DeviceManagement::*;
use qmi_protocol::{ToQmiMsg, QmiMsg};

pub struct QmiClient {

}

impl QmiClient {
    pub fn new() -> Self {
        QmiClient { }
    }

    pub async fn send_msg<T: ToQmiMsg>(&mut self, event: T) -> Result<QmiMsg, Error> {
        //let res = DeviceManagement::EventReportResponse::new();
        Ok(QmiMsg::new())
    }
}


struct QmiModem;

impl QmiModem {
    fn spawn(chan: fasync::Channel) {
        // allocate client ID here and whatnot
        fasync::spawn(
            QmiModemRequestStream::from_channel(chan)
            .try_for_each(Self::handle_request)
            .unwrap_or_else(|e| fx_log_err!("Error running {:?}", e)))
    }

    async fn handle_request(request: QmiModemRequest) -> Result<(), fidl::Error> {
        let client = QmiClient::new();
        match request {
            QmiModemRequest::RequestDataManagementService { service, responder } => {
                let client_id: u32 = 0; // TODO set this via modem
                fx_log_info!("request for Data Management Service from client: {}", client_id);
                // TODO start the service
                                                         //)())); //qmi_msg!(0x0020)));
                Ok(())
                //responder.send(&mut resp)
            }
        }
    }
}

fn main() -> Result<(), Error> {
    syslog::init_with_tags(&["qmi-modem"]).expect("Can't init logger");
    fx_log_info!("Starting qmi-modem...");

    let mut executor = fasync::Executor::new().context("Error creating executor")?;

    let server = ServicesServer::new()
        .add_service((QmiModemMarker::NAME, move |chan: fasync::Channel| {
            fx_log_info!("client connecting to QMI modem");
            QmiModem::spawn(chan)
        })).start()?;

    let client = QmiClient::new();

    let test_fut = async {
        let qmi_resp: EventReportResponse = await!(client.send_msg(EventReport::new()))?.try_into()?;
                                                   //.unwrap().try_into().unwrap());//Some(true), Some(3))));
//        let response = await!(DeviceManagement::set_event_report(&mut client, Some(false), Some(0))); //&client, 0, 0, 0));
        Ok::<(), Error>(())
    };

    let x = executor.run_singlethreaded(test_fut);
    Ok(())
}
