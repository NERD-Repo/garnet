// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#![deny(warnings)]
#![feature(async_await, await_macro, futures_api, pin, arbitrary_self_types)]

extern crate fuchsia_async as fasync;
#[macro_use]
extern crate fuchsia_bluetooth as bt;
extern crate fuchsia_zircon as zx;
#[macro_use]
extern crate log;
extern crate futures;

use fuchsia_app::server::ServicesServer;
use futures::TryFutureExt;
use crate::bt::util;
use failure::{Error, ResultExt};
use fidl::endpoints2::{ServerEnd, ServiceMarker};
use fidl_fuchsia_bluetooth_control::ControlMarker;
use fidl_fuchsia_bluetooth_gatt::Server_Marker;
use fidl_fuchsia_bluetooth_le::{CentralMarker, PeripheralMarker};
use futures::FutureExt;
use parking_lot::RwLock;
use std::sync::Arc;

mod control_service;
mod host_device;
mod host_dispatcher;
mod logger;

use crate::host_dispatcher::*;

const MAX_LOG_LEVEL: log::LevelFilter = log::LevelFilter::Info;
static LOGGER: logger::Logger = logger::Logger;

fn main() -> Result<(), Error> {
    let _ = log::set_logger(&LOGGER);
    log::set_max_level(MAX_LOG_LEVEL);

    info!("Starting bt-gap...");

    let mut executor = fasync::Executor::new().context("Error creating executor")?;
    let hd = Arc::new(RwLock::new(HostDispatcher::new()));

    make_clones!(hd => host_hd, control_hd, central_hd, peripheral_hd, gatt_hd);
    let host_watcher = watch_hosts(host_hd);

    let server = ServicesServer::new()
        .add_service((ControlMarker::NAME, move |chan: fasync::Channel| {
            trace!("Spawning Control Service");
            fasync::spawn(control_service::make_control_service(control_hd.clone(), chan).unwrap_or_else(|e| {
                eprintln!("Error running control service: {:?}", e)
            }))
        }))
        .add_service((CentralMarker::NAME, move |chan: fasync::Channel| {
            trace!("Connecting Control Service to Adapter");
            let central_hd = central_hd.clone();
            fasync::spawn(async move {
                let adapter = await!(HostDispatcher::get_active_adapter(central_hd.clone())).unwrap();
                let remote = ServerEnd::<CentralMarker>::new(chan.into());
                if let Some(adapter) = adapter {
                    let _ = adapter.read().get_host().request_low_energy_central(remote);
                }
            })
        }))
        .add_service((PeripheralMarker::NAME, move |chan: fasync::Channel| {
            trace!("Connecting Peripheral Service to Adapter");
            let hd = peripheral_hd.clone();
            fasync::spawn(async move {
                let adapter = await!(HostDispatcher::get_active_adapter(hd.clone())).unwrap();
                let remote = ServerEnd::<PeripheralMarker>::new(chan.into());
                if let Some(adapter) = adapter {
                    let _ = adapter.read().get_host().request_low_energy_peripheral(remote);
                }
            })
        }))
        .add_service((Server_Marker::NAME, move |chan: fasync::Channel| {
            trace!("Connecting Gatt Service to Adapter");
            let hd = gatt_hd.clone();
            fasync::spawn(async move {
                let adapter = await!(HostDispatcher::get_active_adapter(hd.clone())).unwrap();
                let remote = ServerEnd::<Server_Marker>::new(chan.into());
                if let Some(adapter) = adapter {
                    let _ = adapter.read().get_host().request_gatt_server_(remote);
                }
            })
        }))
        .start()
        .context("error starting bt-gap service")?;

    executor.run_singlethreaded(server.join(host_watcher));
    Ok(())
}
