// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! qmi-ctl is used for interacting with devices that expose a QMI RIL over
//! FIDL on Fuchsia.
//!
//! Ex: run qmi-ctl -d /dev/class/qmi-transport/000
//!
//! Future support for connecting through modem-mgr instead of owning the
//! modem service is planned. A REPL is also planned as the FIDL interfaces
//! evolve.

#![feature(async_await, await_macro, futures_api)]
#![deny(warnings)]

use failure::{format_err, Error, ResultExt};
use fidl::endpoints::create_proxy;
use fidl_fuchsia_telephony_qmi::QmiModemMarker;
use fuchsia_app::client::Launcher;
use fuchsia_async as fasync;
use qmi;
use std::env;
use std::fs::File;

pub fn main() -> Result<(), Error> {
    let mut exec = fasync::Executor::new().context("error creating event loop")?;
    let (_client_proxy, client_server) = create_proxy()?;

    let args: Vec<String> = env::args().collect();

    // TODO more advanced arg parsing
    if args.len() != 3 || args[1] != "-d" {
        eprintln!("qmi-ctl -d <qmi-transport-device path>");
        ::std::process::exit(1);
    }

    let launcher = Launcher::new().context("Failed to open launcher service")?;
    let app = launcher
        .launch(String::from("qmi-modem"), None)
        .context("Failed to launch qmi-modem service")?;
    let qmi_modem = app.connect_to_service(QmiModemMarker)?;

    let path = &args[2];

    let file = File::open(&path)?;
    let chan = qmi::connect_transport_device(&file)?;

    let client_fut = async {
        let connected_transport = await!(qmi_modem.connect_transport(chan))?;
        if connected_transport {
            let client_res = await!(qmi_modem.connect_client(client_server))?;
            if client_res {
                return Ok(());
            }
        }
        Err(format_err!("Failed to request modem or client"))
    };
    exec.run_singlethreaded(client_fut)
}
