// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#![deny(warnings)]

extern crate failure;
extern crate fidl;
extern crate fuchsia_app;
extern crate fuchsia_zircon as zx;
extern crate futures;
extern crate mxruntime;
extern crate mxruntime_sys;
extern crate tokio_core;
extern crate tokio_fuchsia;
extern crate fdio;

use failure::{Error, ResultExt};
use fuchsia_app::server::ServicesServer;
use tokio_core::reactor;

fn main() {
    if let Err(e) = main_ds() {
        eprintln!("DeviceSetting: Error: {:?}", e);
    }
}

// TODO(anmittal): Use log crate and use that for logging
fn main_ds() -> Result<(), Error> {
    let mut core = reactor::Core::new().context("unable to create core")?;
    let server = ServicesServer::new()
    .start(&core.handle())
    .map_err(|e| e.context("error starting service server"))?;
    Ok(core.run(server).context("running server")?)

}

