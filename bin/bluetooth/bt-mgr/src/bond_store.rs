// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use app::client::App;
use failure::{Error, ResultExt};
use futures::future::ok as fok;
use futures::{Future, FutureExt, StreamExt};
use std::fs::File;
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::{thread, time};
use toml;

use app::client::Launcher;
use fidl_bluetooth_bonder::{BonderEvent, BonderEventStream, BonderMarker, BondingData};
use fidl_bluetooth_control::ControlMarker;
use fidl_bluetooth_gatt::Server_Marker;
use fidl_bluetooth_low_energy::{CentralMarker, PeripheralMarker};

use bond_defs::BondMap;

static BT_MGR_DIR: &'static str = "data/bt-mgr";

pub struct BondStore {
    bonds: BondMap,
    bond_store: File,
}

impl BondStore {
    pub fn load_store() -> Result<Self, Error> {
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

    pub fn add(&mut self, local_id: String, bond_data: BondingData) {
        self.bonds.inner_mut().entry(local_id)
            //            TODO look up syntax, push if not there
//            .or_insert(VecBondingData{inner: vec![]})
            .and_modify(move |entry| {
                entry.inner.push(bond_data);
            });
        let _ = self.save_state();
    }

    pub fn bonds(&self) -> &BondMap {
        &self.bonds
    }

    pub fn save_state(&mut self) -> Result<(), Error> {
        let toml = toml::to_string_pretty(&self.bonds)?;
        self.bond_store.write_all(toml.as_bytes())?;
        self.bond_store.sync_data()?;
        Ok(())
    }
}

pub fn bond(bond_store: Arc<Mutex<BondStore>>, bt_gap: App) -> Result<(), Error> {
    let bond_svc = bt_gap.connect_to_service(BonderMarker)?;

    let bond_store = bond_store.lock().unwrap();
    for (bond_key, bond_data) in bond_store.bonds.inner().iter() {
        // TODO make the iter work
        //        bond_svc.add_bonded_devices(bond_key, bond_data.into_iter());
    }

    Ok(())
}

pub fn watch_bonds(
    bond_store: Arc<Mutex<BondStore>>,
) -> impl Future<Item = BonderEventStream, Error = Error> {
    let launcher = Launcher::new()
        .context("Failed to open launcher service")
        .unwrap();
    let app = launcher
        .launch(String::from("bt-gap"), None)
        .context("Failed to launch bt-gap (bluetooth) service")
        .unwrap();
    thread::sleep(time::Duration::from_millis(2000));
    let app = app.connect_to_service(BonderMarker);
    let stream = app.unwrap().take_event_stream();
    stream
        .for_each(move |evt| {
            match evt {
                BonderEvent::OnNewBondingData { local_id, data } => {
                    let mut bond_store = bond_store.lock().unwrap();
                    bond_store.add(local_id, data)
                }
            }
            fok(())
        })
        .err_into()
}
