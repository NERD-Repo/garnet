// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! ril-ctl is used for interacting with devices that expose the standard
//! Fuchsia RIL (FRIL)
//!
//! Ex: run ril-ctl -d /dev/class/ril-transport/000
//!
//! Future support for connecting through modem-mgr instead of owning the
//! modem service is planned. A REPL is also planned as the FIDL interfaces
//! evolve.

#![feature(async_await, await_macro, futures_api, pin)]
#![deny(warnings)]

use {
    crate::commands::{Cmd, ReplControl},
    failure::{Error, ResultExt},
    fidl_fuchsia_telephony_ril::{RadioInterfaceLayerMarker, RadioInterfaceLayerProxy, RadioPowerState},
    fuchsia_app::client::Launcher,
    fuchsia_async::{self as fasync, futures::select},
    futures::TryFutureExt,
    pin_utils::pin_mut,
    qmi,
    std::{env, fs::File},
};

mod commands;
mod repl;

static PROMPT: &str = "\x1b[35mril>\x1b[0m ";

async fn get_imei<'a>(
    _args: &'a [&'a str], ril_modem: &'a RadioInterfaceLayerProxy,
) -> Result<String, Error> {
    let resp = await!(ril_modem.get_device_identity())?;
    Ok(resp)
}

async fn get_power<'a>(
    _args: &'a [&'a str], ril_modem: &'a RadioInterfaceLayerProxy,
) -> Result<String, Error> {
    match await!(ril_modem.radio_power_status())? {
        RadioPowerState::On => Ok(String::from("radio on")),
        RadioPowerState::Off => Ok(String::from("radio off")),
    }
}

async fn handle_cmd(
    ril_modem: &RadioInterfaceLayerProxy, line: String,
) -> Result<ReplControl, Error> {
    let components: Vec<_> = line.trim().split_whitespace().collect();
    if let Some((raw_cmd, args)) = components.split_first() {
        let cmd = raw_cmd.parse();
        let res = match cmd {
            Ok(Cmd::PowerStatus) => await!(get_power(args, &ril_modem)),
            Ok(Cmd::Imei) => await!(get_imei(args, &ril_modem)),
            Ok(Cmd::Help) => Ok(Cmd::help_msg().to_string()),
            Ok(Cmd::Exit) | Ok(Cmd::Quit) => return Ok(ReplControl::Break),
            Err(_) => Ok(format!("\"{}\" is not a valid command", raw_cmd)),
        }?;
        if res != "" {
            println!("{}", res);
        }
    }

    Ok(ReplControl::Continue)
}

pub fn main() -> Result<(), Error> {
    let mut exec = fasync::Executor::new().context("error creating event loop")?;
    let args: Vec<String> = env::args().collect();

    // TODO more advanced arg parsing
    // TODO chose what service we are launching from CLI (ex: ril-at)
    if args.len() != 3 || args[1] != "-d" {
        eprintln!("ril-ctl -d <ril-transport-device path>");
        ::std::process::exit(1);
    }

    let launcher = Launcher::new().context("Failed to open launcher service")?;
    let app = launcher
        .launch(String::from("ril-qmi"), None)
        .context("Failed to launch ril-qmi service")?;
    let ril_modem = app.connect_to_service(RadioInterfaceLayerMarker)?;

    let path = &args[2];
    let file = File::open(&path)?;
    let chan = qmi::connect_transport_device(&file)?;

    let client_fut = async {
        await!(ril_modem.connect_transport(chan))?;
        let repl =
            repl::run(ril_modem).unwrap_or_else(|e| eprintln!("REPL failed unexpectedly {:?}", e));
        pin_mut!(repl);
        select! {
            repl => Ok(()),
            // TODO(bwb): events loop future
        }
    };

    exec.run_singlethreaded(client_fut)
}
