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
extern crate fuchsia_app as app;
extern crate fuchsia_async as async;
extern crate fuchsia_zircon as zx;
extern crate futures;
extern crate parking_lot;
#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate toml;

use std::io::Read;
use std::path::PathBuf;

use fidl::endpoints2::ServiceMarker;
use std::{thread, time};

use app::client::App;
use std::sync::{Arc, Mutex};

use std::fs::File;
use std::fs::OpenOptions;

use std::io::Write;

use app::client::Launcher;
use app::server::ServicesServer;
use failure::{Error, ResultExt};
use fidl_bluetooth_bonder::BonderMarker;
use fidl_bluetooth_control::ControlMarker;

mod bond_defs;
mod logger;

use bond_defs::*;

const MAX_LOG_LEVEL: log::LevelFilter = log::LevelFilter::Info;
static LOGGER: logger::Logger = logger::Logger;

static BT_MGR_DIR: &'static str = "data/bt-mgr";

struct BondStore {
    bonds: BondMap,
    bond_store: File,
}

impl BondStore {
    fn load_store() -> Result<Self, Error> {
        let store_path: PathBuf = [BT_MGR_DIR, "/bonds.toml"].iter().collect();

        let mut bond_store = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(store_path)?;

        let mut contents = String::new();
        bond_store
            .read_to_string(&mut contents)
            .expect("The bond storage file is corrupted");
        let bonds: BondMap = toml::from_str(contents.as_str()).unwrap();

        Ok(BondStore { bonds, bond_store })
    }

    fn save_state(&mut self) -> Result<(), Error> {
        let toml = toml::to_string_pretty(&self.bonds)?;
        self.bond_store.write_all(toml.as_bytes())?;
        self.bond_store.sync_data()?;
        Ok(())
    }
}

fn bond(_bond_store: Arc<Mutex<BondStore>>, bt_gap: App) -> Result<(), Error> {
    let _bond_svc = bt_gap.connect_to_service(BonderMarker)?;

    //bond_svc.add_bonded_devices();
    Ok(())
}

fn main() -> Result<(), Error> {
    let _ = log::set_logger(&LOGGER);
    log::set_max_level(MAX_LOG_LEVEL);

    info!("Starting bt-mgr...");
    let mut executor = async::Executor::new().context("Error creating executor")?;

    let bond_store = Arc::new(Mutex::new(BondStore::load_store()?));
    let server = ServicesServer::new()
        .add_service((ControlMarker::NAME, move |chan: async::Channel| {
            info!("Passing Control Handle to bt-gap");
            let launcher = Launcher::new()
                .context("Failed to open launcher service")
                .unwrap();
            let app = launcher
                .launch(String::from("bt-gap"), None)
                .context("Failed to launch bt-gap (bluetooth) service")
                .unwrap();
            thread::sleep(time::Duration::from_millis(2000));
            let _ = app.pass_to_service(ControlMarker, chan.into());
            let _ = bond(bond_store.clone(), app);
        }))
        .start()
        .map_err(|e| e.context("error starting service server"))?;

    executor
        .run_singlethreaded(server)
        .context("bt-mgr failed to execute future")
        .map_err(|e| e.into())
}
