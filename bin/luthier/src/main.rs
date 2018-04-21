// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#![deny(warnings)]
#![feature(conservative_impl_trait)]

//#[macro_use]
extern crate failure;
// #[macro_use]
extern crate fdio;
extern crate fidl;
extern crate fidl_luthier;
extern crate fuchsia_app as app;
extern crate fuchsia_async as async;
extern crate fuchsia_zircon as zx;
extern crate futures;
extern crate parking_lot;

#[macro_use]
extern crate structopt;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use structopt::StructOpt;

use app::server::ServicesServer;
use failure::{Error, ResultExt};
use fidl::endpoints2::ServiceMarker;
use fidl_luthier::{Luthier, LuthierImpl, LuthierMarker};
use futures::prelude::*;
use parking_lot::Mutex;

type SimpleStore = Arc<Mutex<HashMap<String, String>>>;

#[derive(StructOpt, Debug)]
#[structopt(name = "luthier")]
struct Opt {
    #[structopt(short = "v", long = "verbose", parse(from_occurrences))]
    verbose: u8,

    #[structopt(short = "d", long = "directory", parse(from_os_str))]
    startup_directory: Option<PathBuf>,

    #[structopt(name = "FILE", parse(from_os_str))]
    fidl_files: Vec<PathBuf>,
}

fn register_ir(store: &mut SimpleStore, interface: String, ir: String) -> bool {
    let mut map = store.lock();
    if map.contains_key(&interface) {
        // TODO trigger cache invalidation on clients
    }
    let res = map.insert(interface, ir);

    match res {
        Some(_) => true,
        None => false,
    }
}

fn request_ir(store: &mut SimpleStore, interface: String) -> Option<String> {
    let map = store.lock();
    map.get(&interface).map(|s| s.clone())
}

fn spawn_luthier_server(simple_store: SimpleStore, chan: async::Channel) {
    async::spawn(
        LuthierImpl {
            state: simple_store,
            register_ir: |store, interface, fidl_json_ir, res| {
                let mut status = register_ir(store, interface, fidl_json_ir);
                res.send(&mut status)
                    .into_future()
                    .map(|_| println!("FIDR IR registered successfully")) // TODO use log crate
                    .recover(|e| eprintln!("error sending response: {:?}", e))
            },
            request_ir: |store, interface, res| {
                println!("Received Ir request for {}", interface);
                let mut resp = request_ir(store, interface);
                res.send(&mut resp)
                    .into_future()
                    .map(|_| println!("FIDL IR response sent successfully"))
                    .recover(|e| eprintln!("error sending response: {:?}", e))
            },
        }.serve(chan)
            .recover(|e| eprintln!("error running echo server: {:?}", e)),
    )
}

fn startup_luthier(simple_store: SimpleStore) -> Result<(), Error> {
    let mut executor = async::Executor::new().context("Error creating executor")?;

    let fut = ServicesServer::new()
        .add_service((LuthierMarker::NAME, move |chan| spawn_luthier_server(simple_store.clone(), chan)))
        .start()
        .context("Error starting Luthier, the fidl introspector")?;

    // TODO make multithreaded!
    executor
        .run_singlethreaded(fut)
        .context("failed to execute Luthier server future")?;
    Ok(())
}

fn main() {
    let opt = Opt::from_args();
    let simple_store: SimpleStore = Arc::new(Mutex::new(HashMap::new()));
    let mut s = simple_store.clone();

    let control_ir = include_str!(concat!(env!("FIDL_GEN_ROOT"), "/garnet/public/lib/bluetooth/fidl/bluetooth_control.fidl.json"));

    register_ir(&mut s, "bluetooth::control::Control".to_string(), control_ir.to_string());

    startup_luthier(simple_store).unwrap(); //TODO error
}
