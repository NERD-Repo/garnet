// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#![deny(warnings)]
#![feature(
    futures_api,
    pin,
    arbitrary_self_types,
    await_macro,
    async_await
)]

use failure::{Error, ResultExt};
use fidl::endpoints::{ServiceMarker};
use fidl_fuchsia_bluetooth_bredr::ProfileMarker;
use fidl_fuchsia_bluetooth_control::BondingMarker;
use fidl_fuchsia_bluetooth_control::ControlMarker;
use fidl_fuchsia_bluetooth_gatt::Server_Marker;
use fidl_fuchsia_bluetooth_le::{CentralMarker, PeripheralMarker};
use fuchsia_app::server::ServicesServer;
use fuchsia_async as fasync;
use fuchsia_bluetooth::util;
use fuchsia_syslog::{self as syslog, fx_log, fx_log_info};
use futures::TryFutureExt;

mod services;

mod host_device;
mod host_dispatcher;

use crate::host_dispatcher::*;

fn main() -> Result<(), Error> {
    syslog::init_with_tags(&["bt-gap"]).expect("Can't init logger");
    fx_log_info!("Starting bt-gap...");

    let mut executor = fasync::Executor::new().context("Error creating executor")?;

    let hd = HostDispatcher::new();

    let host_watcher = watch_hosts(hd.clone());

    let central_hd = hd.clone();
    let bonding_hd = hd.clone();
    let control_hd = hd.clone();
    let peripheral_hd = hd.clone();
    let profile_hd = hd.clone();
    let gatt_hd = hd.clone();

    let server = ServicesServer::new()
        .add_service((ControlMarker::NAME, move |chan: fasync::Channel| {
            control_service(control_hd.clone(), chan)
        })).add_service((BondingMarker::NAME, move |chan: fasync::Channel| {
            bonding_service(bonding_hd.clone(), chan)
        })).add_service((CentralMarker::NAME, move |chan: fasync::Channel| {
            fx_log_info!("Connecting CentralService to Adapter");
            fasync::spawn(central_hd.clone().request_host_service(chan, HostService::LeCentral))
        })).add_service((PeripheralMarker::NAME, move |chan: fasync::Channel| {
            fx_log_info!("Connecting Peripheral Service to Adapter");
            fasync::spawn(peripheral_hd.clone().request_host_service(chan, HostService::LePeripheral))
        })).add_service((ProfileMarker::NAME, move |chan: fasync::Channel| {
            fx_log_info!("Connecting Profile Service to Adapter");
            fasync::spawn(profile_hd.clone().request_host_service(chan, HostService::Profile))
        })).add_service((Server_Marker::NAME, move |chan: fasync::Channel| {
            fx_log_info!("Connecting Gatt Service to Adapter");
            fasync::spawn(gatt_hd.clone().request_host_service(chan, HostService::LeGatt))
        })).start()?;

    executor
        .run_singlethreaded(server.try_join(host_watcher))
        .map(|_| ())
}

fn control_service(hd: HostDispatcher, chan: fasync::Channel) {
    fx_log_info!("Spawning Control Service");
    fasync::spawn(
        services::start_control_service(hd.clone(), chan)
            .unwrap_or_else(|e| eprintln!("Failed to spawn {:?}", e)),
    )
}

fn bonding_service(hd: HostDispatcher, chan: fasync::Channel) {
    fx_log_info!("Spawning Bonding Service");
    fasync::spawn(
        services::start_bonding_service(hd.clone(), chan)
            .unwrap_or_else(|e| eprintln!("Failed to spawn {:?}", e)),
    )
}
