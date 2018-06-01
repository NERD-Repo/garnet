#![allow(infoings)]
#![feature(proc_macro, conservative_impl_trait, generators)]

//#[macro_use]
extern crate failure;
// #[macro_use]
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
//#[macro_use]
extern crate serde;
extern crate toml;
//extern crate structopt;
// #[macro_use]
//extern crate serde_derive;

use futures::{future, FutureExt};
use futures::IntoFuture;
use futures::Never;
use futures::Future;

use std::mem;

use std::{thread, time};
use fidl::endpoints2::{Proxy, ServiceMarker};

use app::client::connect_to_service;
//use app::client::pass_channel_to_service;
use std::sync::{Arc, Mutex};

use std::fs::File;
use std::fs::OpenOptions;

use std::io::Write;

use fidl_bluetooth_bonder::{Bonder, BonderImpl, BonderMarker, BonderProxy};
use fidl_bluetooth_control::{Control, ControlImpl, ControlMarker, ControlProxy};
use failure::{Error, ResultExt};
use app::server::ServicesServer;
use app::client::Launcher;
use std::collections::HashMap;

mod logger;

const MAX_LOG_LEVEL: log::LevelFilter = log::LevelFilter::Info;
static LOGGER: logger::Logger = logger::Logger;
//static BT_MGR_DIR: &'static str = "data/btmgr";

struct BondStore {
    bonds: HashMap<String, Vec<u8>>,
    //file: File,
}

impl BondStore {
    fn save_state(&mut self) -> Result<(), Error> {
        let string = toml::to_string(&self.bonds)?;
        //self.file.write_all(string.as_bytes())?;
        //self.file.sync_data()?;
        Ok(())
    }
}

fn main() -> Result<(), Error> {
    let _ = log::set_logger(&LOGGER);
    log::set_max_level(MAX_LOG_LEVEL);

    info!("Starting bt-mgr...");
    let mut executor = async::Executor::new().context("Error creating executor")?;

    let server = ServicesServer::new()
        .add_service((ControlMarker::NAME, move |chan: async::Channel| {
            info!("launch");
            let launcher = Launcher::new().context("Failed to open launcher service").unwrap();
            info!("STHEu");
            let app = launcher.launch(String::from("bt-gap"), None).context("Failed to launch bt-gap (bluetooth) service").unwrap();
            thread::sleep(time::Duration::from_millis(10000));
            info!("aseouthSTHEu");
            app.pass_to_service(ControlMarker, chan.into());
            thread::sleep(time::Duration::from_millis(4000));
            info!("STHEue");
        }))
        .start()
        .map_err(|e| e.context("error starting service server"))?;

    executor
        .run_singlethreaded(server)
        .context("bt-mgr failed to execute future")
        .map_err(|e| e.into())
}
