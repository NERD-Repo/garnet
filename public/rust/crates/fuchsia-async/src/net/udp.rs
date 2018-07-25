// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use futures::{task, Future, Poll, ready};
use std::io;
use std::net::{self, SocketAddr};
use std::ops::Deref;
use std::os::unix::io::AsRawFd;

use crate::net::{set_nonblock, EventedFd};

/// An I/O object representing a UDP socket.
pub struct UdpSocket(EventedFd<net::UdpSocket>);

impl Deref for UdpSocket {
    type Target = EventedFd<net::UdpSocket>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl UdpSocket {
    pub fn bind(addr: &SocketAddr) -> io::Result<UdpSocket> {
        let socket = net::UdpSocket::bind(addr)?;
        UdpSocket::from_socket(socket)
    }

    pub fn from_socket(socket: net::UdpSocket) -> io::Result<UdpSocket> {
        set_nonblock(socket.as_raw_fd())?;

        unsafe { Ok(UdpSocket(EventedFd::new(socket)?)) }
    }


    pub fn recv_from<'a>(&'a self, buf: &'a mut [u8])
        -> impl Future<Output = Result<(usize, SocketAddr), io::Error>> + 'a
    {
        futures::future::poll_fn(move |cx| self.async_recv_from(buf, cx))
    }

    pub fn async_recv_from(
        &self, buf: &mut [u8], cx: &mut task::Context,
    ) -> Poll<Result<(usize, SocketAddr), io::Error>> {
        ready!(EventedFd::poll_readable(&self.0, cx));
        match self.0.as_ref().recv_from(buf) {
            Err(e) => {
                if e.kind() == io::ErrorKind::WouldBlock {
                    self.0.need_read(cx);
                    Poll::Pending
                } else {
                    Poll::Ready(Err(e))
                }
            }
            Ok((size, addr)) => Poll::Ready(Ok((size, addr))),
        }
    }

    pub fn send_to<'a>(
        &'a self, buf: &'a [u8], addr: SocketAddr,
    ) -> impl Future<Output = Result<(), io::Error>> + 'a {
        futures::future::poll_fn(move |cx| self.async_send_to(buf, addr, cx))
    }

    pub fn async_send_to(
        &self, buf: &[u8], addr: SocketAddr, cx: &mut task::Context,
    ) -> Poll<Result<(), io::Error>> {
        ready!(EventedFd::poll_writable(&self.0, cx));
        match self.0.as_ref().send_to(buf, addr) {
            Err(e) => {
                if e.kind() == io::ErrorKind::WouldBlock {
                    self.0.need_write(cx);
                    Poll::Pending
                } else {
                    Poll::Ready(Err(e))
                }
            }
            Ok(_) => Poll::Ready(Ok(())),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::Executor;
    use super::*;

    #[test]
    fn send_recv() {
        let mut exec = Executor::new().expect("could not create executor");

        let addr = "127.0.0.1:29995".parse().unwrap();
        let buf = b"hello world";
        let socket = UdpSocket::bind(&addr).expect("could not create socket");
        let fut = async {
            await!(socket.send_to(buf, addr))?;
            let mut recvbuf = [0u8; 11];
            let (received, sender) = await!(socket.recv_from(&mut recvbuf))?;
            assert_eq!(addr, sender);
            assert_eq!(received, buf.len());
            assert_eq!(buf, &recvbuf);
            Ok::<(), io::Error>(())
        };

        exec.run_singlethreaded(fut).expect("failed to run udp socket test");
    }
}
