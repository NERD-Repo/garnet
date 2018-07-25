// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#![feature(async_await, await_macro, futures_api)]

use fuchsia_app::server::ServicesServer;
use fuchsia_async as fasync;
use failure::{Error, ResultExt};
use futures::prelude::*;
use fidl::endpoints2::{ServiceMarker, RequestStream};
use fidl_fidl_examples_echo::{EchoMarker, EchoRequest, EchoRequestStream};
use std::env;

async fn run_echo_server(chan: fasync::Channel, quiet: bool) -> Result<(), Error> {
    let mut requests = EchoRequestStream::from_channel(chan);
    while let Some(res) = await!(requests.next()) {
        let EchoRequest::EchoString { value, responder } = res?;
        if !quiet {
            println!("Received echo request for string {:?}", value);
        }
        responder.send(value.as_ref().map(|s| &**s))?;
        if !quiet {
           println!("echo response sent successfully");
        }
    }
    Ok(())
}

fn spawn_echo_server(chan: fasync::Channel, quiet: bool) {
    fasync::spawn(run_echo_server(chan, quiet).unwrap_or_else(|e|
        eprintln!("Error running echo server: {:?}", e)))
}

fn main() -> Result<(), Error> {
    let mut executor = fasync::Executor::new().context("Error creating executor")?;
    let quiet = env::args().any(|arg| arg == "-q");

    let fut = ServicesServer::new()
                .add_service((EchoMarker::NAME, move |chan| spawn_echo_server(chan, quiet)))
                .start()
                .context("Error starting echo services server")?;

    executor.run_singlethreaded(fut).context("failed to execute echo future")?;
    Ok(())
}
