// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#![feature(proc_macro, generators)]
//#![deny(warnings)]

extern crate failure;
extern crate fdio;
extern crate fidl;
extern crate fidl_bluetooth_bonder;
extern crate fidl_bluetooth_control;
extern crate fidl_bluetooth_gatt;
extern crate fidl_bluetooth_low_energy;
extern crate fuchsia_app as app;
extern crate fuchsia_async as async;
extern crate fuchsia_zircon as zx;
#[macro_use]
extern crate fuchsia_bluetooth as bt;
extern crate futures;
extern crate parking_lot;
#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate toml;

use fidl::endpoints2::ServiceMarker;
use std::{thread, time};

use app::client::App;
use std::sync::{Arc, Mutex};

use futures::future::ok as fok;
use futures::{Future, FutureExt, StreamExt};

use app::client::Launcher;
use app::server::ServicesServer;
use failure::{Error, ResultExt};
use fidl_bluetooth_control::ControlMarker;
use fidl_bluetooth_gatt::Server_Marker;
use fidl_bluetooth_low_energy::{CentralMarker, PeripheralMarker};

mod bond_defs;
mod bond_store;
mod logger;

use bond_defs::*;
use bond_store::BondStore;

const MAX_LOG_LEVEL: log::LevelFilter = log::LevelFilter::Info;
static LOGGER: logger::Logger = logger::Logger;

fn launch_bt_gap<S: ServiceMarker>(
    marker: S, bond_store: Arc<Mutex<BondStore>>, chan: async::Channel,
) {
    let launcher = Launcher::new()
        .context("Failed to open launcher service")
        .unwrap();
    let app = launcher
        .launch(String::from("bt-gap"), None)
        .context("Failed to launch bt-gap (bluetooth) service")
        .unwrap();
    // TODO check if we need to launch a service?
    thread::sleep(time::Duration::from_millis(2000));
    let _ = app.pass_to_service(marker, chan.into());
    // TODO cap the number of times this is done?
    let _ = bond_store::bond(bond_store.clone(), app);
}

fn main() -> Result<(), Error> {
    let _ = log::set_logger(&LOGGER);
    log::set_max_level(MAX_LOG_LEVEL);

    info!("Starting bt-mgr...");
    let mut executor = async::Executor::new().context("Error creating executor")?;

    let bond_store = Arc::new(Mutex::new(BondStore::load_store()?));
    make_clones!(bond_store => control_bs, central_bs, peripheral_bs, server_bs, bond_bs);

    let bond_watcher = bond_store::watch_bonds(bond_bs);

    let server = ServicesServer::new()
        .add_service((ControlMarker::NAME, move |chan: async::Channel| {
            info!("Passing Control Handle to bt-gap");
            launch_bt_gap(ControlMarker, control_bs.clone(), chan);
        }))
        .add_service((CentralMarker::NAME, move |chan: async::Channel| {
            info!("Passing LE Central Handle to bt-gap");
            launch_bt_gap(CentralMarker, central_bs.clone(), chan);
        }))
        .add_service((PeripheralMarker::NAME, move |chan: async::Channel| {
            info!("Passing Peripheral Handle to bt-gap");
            launch_bt_gap(PeripheralMarker, peripheral_bs.clone(), chan);
        }))
        .add_service((Server_Marker::NAME, move |chan: async::Channel| {
            info!("Passing GATT Handle to bt-gap");
            launch_bt_gap(Server_Marker, server_bs.clone(), chan);
        }))
        .start()
        .map_err(|e| e.context("error starting service server"))?;

    executor
        .run_singlethreaded(server.join(bond_watcher))
        .context("bt-mgr failed to execute future")
        .map(|_| ())
        .map_err(|e| e.into())
}
