// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#![feature(async_await, await_macro)]

extern crate ethernet;
extern crate failure;
extern crate fuchsia_async as fasync;
extern crate fuchsia_zircon as zx;
extern crate futures;

use failure::{Error, ResultExt};
use futures::prelude::*;
use std::env;
use std::fs::File;

fn main() -> Result<(), Error> {
    let vmo = zx::Vmo::create_with_opts(zx::VmoOptions::NON_RESIZABLE, 256 * ethernet::DEFAULT_BUFFER_SIZE as u64)?;

    let mut executor = fasync::Executor::new().context("could not create executor")?;

    let path = env::args().nth(1).expect("missing device argument");
    let dev = File::open(path)?;
    let client = ethernet::Client::new(dev, vmo, ethernet::DEFAULT_BUFFER_SIZE, "eth-rs")?;
    println!("created client {:?}", client);
    println!("info: {:?}", client.info()?);
    println!("status: {:?}", client.get_status()?);
    client.start()?;
    client.tx_listen_start()?;
    let mut stream = client.get_stream();

    let fut = async {
        while let Some(evt) = await!(stream.next()) {
            let evt = evt?;
            println!("{:?}", evt);
            match evt {
                ethernet::Event::Receive(rx) => {
                    let mut buf = [0; 64];
                    let r = rx.read(&mut buf);
                    println!("first {} bytes: {:02x?}", r, &buf[0..r]);
                },
                _ => (),
            }
        }
        Ok(())
    };
    executor.run_singlethreaded(fut)
}
