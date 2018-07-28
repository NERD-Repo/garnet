// Copyright 2017 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#![deny(warnings)]
#![feature(async_await, await_macro, futures_api)]

use failure::{bail, Error, ResultExt};
use fidl_fuchsia_net_oldhttp as http;
use fuchsia_async as fasync;
use fuchsia_zircon as zx;
use futures::prelude::*;
use futures::io::AllowStdIo;

/// Connects to the http service, sends a url request, and prints the response.
fn main() -> Result<(), Error> {
    let url = match std::env::args().nth(1) {
        Some(url) => {
            if url.find("://").is_none() {
                ["http://", &url].concat()
            } else {
                url
            }
        }
        None => {
            println!("usage: {} <url>", std::env::args().nth(0).unwrap());
            return Ok(());
        }
    };

    // Set up fasync executor
    let mut exec = fasync::Executor::new()?;

    let fut = async_main(url);

    //// Run the future to completion
    exec.run_singlethreaded(fut)
}

async fn async_main(url: String) -> Result<(), Error> {
    // Connect to the http service
    let net = fuchsia_app::client::connect_to_service::<http::HttpServiceMarker>()?;

    // Create a UrlLoader instance
    let (s, p) = zx::Channel::create().context("failed to create zx channel")?;
    let proxy = fasync::Channel::from_channel(p).context("failed to make fasync channel")?;

    let loader_server = fidl::endpoints2::ServerEnd::<http::UrlLoaderMarker>::new(s);
    net.create_url_loader(loader_server)?;

    // Send the UrlRequest to fetch the webpage
    let mut req = http::UrlRequest {
        url: url,
        method: String::from("GET"),
        headers: None,
        body: None,
        response_body_buffer_size: 0,
        auto_follow_redirects: true,
        cache_mode: http::CacheMode::Default,
        response_body_mode: http::ResponseBodyMode::Stream,
    };

	let loader_proxy = http::UrlLoaderProxy::new(proxy);
    let resp = await!(loader_proxy.start(&mut req))?;
    if let Some(e) = resp.error {
        let code = e.code;
        bail!("Got error: {} ({})",
            code,
            e.description.unwrap_or("".into()));
    }

    // Print headers
    println!(">>> Headers <<<");
    if let Some(status) = &resp.status_line {
        println!("  {}", status);
    }
    if let Some(hdrs) = &resp.headers {
        for hdr in hdrs {
            println!("  {}={}", hdr.name, hdr.value);
        }
    }

    let mut socket = match resp.body.map(|x| *x) {
        Some(http::UrlBody::Stream(s)) => fasync::Socket::from_socket(s)?,
        Some(http::UrlBody::Buffer(_))
        | Some(http::UrlBody::SizedBuffer(_))
        | None => return Ok(()),
    };

    // stdout is blocking, but we'll pretend it's okay
    println!(">>> Body <<<");

    // Copy the bytes from the socket to stdout
    {
        let mut stdout = AllowStdIo::new(::std::io::stdout());
        await!(socket.copy_into(&mut stdout))?;
    }
    println!("\n>>> EOF <<<");
    Ok(())
}
