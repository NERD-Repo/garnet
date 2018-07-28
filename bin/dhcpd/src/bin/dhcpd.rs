// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.
#![deny(warnings)]
#![feature(async_await, await_macro)]

use fuchsia_async::{Executor, net::UdpSocket};
use failure::{Error, ResultExt};
use dhcp::protocol::{Message, SERVER_PORT};
use dhcp::server::{Server, ServerConfig};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};

/// A buffer size in excess of the maximum allowable DHCP message size.
const BUF_SZ: usize = 1024;

fn main() -> Result<(), Error> {
    println!("dhcpd: starting...");
    let mut exec = Executor::new().context("error creating executor")?;
    let (udp_socket, server) = setup_and_bind_server(Ipv4Addr::new(127, 0, 0, 1))?;
    build_and_run_event_loop(&mut exec, udp_socket, server)
}

fn setup_and_bind_server(server_ip: Ipv4Addr) -> Result<(UdpSocket, Arc<Mutex<Server>>), Error> {
    let addr = SocketAddr::new(IpAddr::V4(server_ip), SERVER_PORT);
    let udp_socket = UdpSocket::bind(&addr).context("unable to bind socket")?;
    let server = build_server(server_ip)?;
    Ok((udp_socket, server))
}

fn build_server(server_ip: Ipv4Addr) -> Result<Arc<Mutex<Server>>, Error> {
    let mut server = Server::new();
    // Placeholder addresses until the server supports loading addresses
    // from a configuration file.
    let addrs = vec![Ipv4Addr::new(192, 168, 0, 2),
                     Ipv4Addr::new(192, 168, 0, 3),
                     Ipv4Addr::new(192, 168, 0, 4)];
    let config = ServerConfig {
        server_ip: server_ip,
        default_lease_time: 0,
        subnet_mask: 24,
    };
    server.add_addrs(addrs);
    server.set_config(config);
    Ok(Arc::new(Mutex::new(server)))
}

fn build_and_run_event_loop(exec: &mut Executor, sock: UdpSocket, server: Arc<Mutex<Server>>) -> Result<(), Error> {
    let event_loop = async {
        loop {
            let mut buf = vec![0u8; BUF_SZ];
            let (received, addr) = await!(sock.recv_from(&mut buf)).context("unable to receive buffer")?;
            println!("dhcpd: received {} bytes", received);
            let msg = Message::from_buffer(&buf)
                .ok_or_else(|| failure::err_msg("dhcpd: unable to parse buffer"))?;
            println!("dhcpd: msg parsed: {:?}", msg);
            // This call should not block because the server is single-threaded.
            let response = server.lock().unwrap().dispatch(msg)
                .ok_or_else(|| failure::err_msg("dhcpd: invalid Message"))?;
            println!("dhcpd: msg dispatched to server {:?}", response);
            let response_buffer = response.serialize();
            println!("dhcpd: response serialized");
            await!(sock.send_to(&response_buffer, addr))
                .context("dhcpd: unable to send response")?;
            println!("dhcpd: response sent. Continuing event loop.");
        }
    };

    println!("dhcpd: starting event loop...");
    let res: Result<(), Error> = exec.run_singlethreaded(event_loop);
    res.context("could not run futures")?;
    println!("dhcpd: shutting down...");
    Ok(())
}
