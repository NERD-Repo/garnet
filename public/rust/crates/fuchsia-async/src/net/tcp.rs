// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use bytes::{Buf, BufMut};
use futures::future::poll_fn;
use futures::io::{AsyncRead, AsyncWrite, Initializer};
use futures::{ready, task, Future, Poll, Stream};
use libc;
use std::io::{self, Read, Write};
use std::mem::PinMut;
use std::net::{self, SocketAddr};
use std::ops::Deref;

use std::os::unix::io::AsRawFd;

use net2::{TcpBuilder, TcpStreamExt};

use crate::net::{set_nonblock, EventedFd};

/// An I/O object representing a TCP socket listening for incoming connections.
///
/// This object can be converted into a stream of incoming connections for
/// various forms of processing.
pub struct TcpListener(EventedFd<net::TcpListener>);

impl Deref for TcpListener {
    type Target = EventedFd<net::TcpListener>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl TcpListener {
    pub fn bind(addr: &SocketAddr) -> io::Result<TcpListener> {
        let sock = match *addr {
            SocketAddr::V4(..) => TcpBuilder::new_v4(),
            SocketAddr::V6(..) => TcpBuilder::new_v6(),
        }?;

        sock.reuse_address(true)?;
        sock.bind(addr)?;
        let listener = sock.listen(1024)?;
        TcpListener::new(listener)
    }

    pub fn new(listener: net::TcpListener) -> io::Result<TcpListener> {
        set_nonblock(listener.as_raw_fd())?;

        unsafe { Ok(TcpListener(EventedFd::new(listener)?)) }
    }

    pub fn accept<'a>(
        &'a mut self,
    ) -> impl Future<Output = io::Result<(TcpStream, SocketAddr)>> + 'a {
        poll_fn(move |cx| self.async_accept(cx))
    }

    pub fn accept_stream(self) -> AcceptStream {
        AcceptStream(self)
    }

    pub fn async_accept(
        &mut self, cx: &mut task::Context,
    ) -> Poll<Result<(TcpStream, SocketAddr), io::Error>> {
        ready!(EventedFd::poll_readable(&self.0, cx));

        match self.0.as_ref().accept() {
            Err(e) => {
                if e.kind() == io::ErrorKind::WouldBlock {
                    self.0.need_read(cx);
                    return Poll::Pending;
                }
                return Poll::Ready(Err(e));
            }
            Ok((sock, addr)) => {
                return match TcpStream::from_stream(sock) {
                    Ok(sock) => Poll::Ready(Ok((sock, addr))),
                    Err(e) => Poll::Ready(Err(e)),
                }
            }
        }
    }

    pub fn from_listener(
        listener: net::TcpListener, _addr: &SocketAddr,
    ) -> io::Result<TcpListener> {
        TcpListener::new(listener)
    }
}

pub struct AcceptStream(TcpListener);

impl Stream for AcceptStream {
    type Item = io::Result<(TcpStream, SocketAddr)>;

    fn poll_next(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Option<Self::Item>> {
        match self.0.async_accept(cx) {
            Poll::Ready(Ok((stream, addr))) => Poll::Ready(Some(Ok((stream, addr)))),
            Poll::Ready(Err(e)) => Poll::Ready(Some(Err(e))),
            Poll::Pending => Poll::Pending,
        }
    }
}

pub struct TcpStream {
    stream: EventedFd<net::TcpStream>,
}

impl Deref for TcpStream {
    type Target = EventedFd<net::TcpStream>;

    fn deref(&self) -> &Self::Target {
        &self.stream
    }
}

impl TcpStream {
    pub fn connect(addr: SocketAddr) -> io::Result<impl Future<Output = io::Result<TcpStream>>> {
        let sock = match addr {
            SocketAddr::V4(..) => TcpBuilder::new_v4(),
            SocketAddr::V6(..) => TcpBuilder::new_v6(),
        }?;

        let stream = sock.to_tcp_stream()?;
        set_nonblock(stream.as_raw_fd())?;
        // This is safe because the file descriptor for stream will live as long as the TcpStream.
        let stream = unsafe { EventedFd::new(stream)? };

        Ok(async move {
            await!(poll_fn(|cx| match stream.as_ref().connect(addr) {
                Err(e) => {
                    if e.raw_os_error() == Some(libc::EINPROGRESS) {
                        stream.need_write(cx);
                        Poll::Pending
                    } else {
                        Poll::Ready(Err(e))
                    }
                }
                Ok(()) => Poll::Ready(Ok(())),
            }))?;

            await!(poll_fn(|cx| stream.poll_writable(cx)));

            Ok(TcpStream { stream })
        })
    }

    pub fn read_buf<B: BufMut>(
        &self, buf: &mut B, cx: &mut task::Context,
    ) -> Poll<io::Result<usize>> {
        match (&self.stream).as_ref().read(unsafe { buf.bytes_mut() }) {
            Ok(n) => {
                unsafe {
                    buf.advance_mut(n);
                }
                Poll::Ready(Ok(n))
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                self.stream.need_read(cx);
                Poll::Pending
            }
            Err(e) => Poll::Ready(Err(e)),
        }
    }

    pub fn write_buf<B: Buf>(
        &self, buf: &mut B, cx: &mut task::Context,
    ) -> Poll<io::Result<usize>> {
        match (&self.stream).as_ref().write(buf.bytes()) {
            Ok(n) => {
                buf.advance(n);
                Poll::Ready(Ok(n))
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                self.stream.need_write(cx);
                Poll::Pending
            }
            Err(e) => Poll::Ready(Err(e)),
        }
    }

    fn from_stream(stream: net::TcpStream) -> io::Result<TcpStream> {
        set_nonblock(stream.as_raw_fd())?;

        // This is safe because the file descriptor for stream will live as long as the TcpStream.
        let stream = unsafe { EventedFd::new(stream)? };

        Ok(TcpStream { stream })
    }
}

impl AsyncRead for TcpStream {
    unsafe fn initializer(&self) -> Initializer {
        // This is safe because `zx::Socket::read` does not examine
        // the buffer before reading into it.
        Initializer::nop()
    }

    fn poll_read(&mut self, cx: &mut task::Context, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        self.stream.poll_read(cx, buf).map_err(|e| e.into())
    }

    // TODO: override poll_vectored_read and call readv on the underlying stream
}

impl AsyncWrite for TcpStream {
    fn poll_write(&mut self, cx: &mut task::Context, buf: &[u8]) -> Poll<io::Result<usize>> {
        self.stream.poll_write(cx, buf).map_err(|e| e.into())
    }

    fn poll_flush(&mut self, _: &mut task::Context) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(&mut self, _: &mut task::Context) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    // TODO: override poll_vectored_write and call writev on the underlying stream
}
