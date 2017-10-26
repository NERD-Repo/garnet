// Copyright 2017 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use libc::PATH_MAX;

use std::sync::Arc;
use tokio_core;
use zx;
use tokio_fuchsia;
use futures;
use futures::Future;
use std;
use std::io;
use std::os::unix::ffi::OsStrExt;
use fdio;
use libc;

// validate open flags that are not vnode specific
fn prevalidate_flags(flags: i32) -> Result<(), zx::Status> {
    let f = flags & libc::O_ACCMODE;
    if f == libc::O_PATH {
        return Ok(());
    }
    if f & libc::O_RDONLY != 0 {
        if flags & libc::O_TRUNC != 0 {
            return Err(zx::Status::INVALID_ARGS)
        }
        return Ok(());
    }
    if f == libc::O_WRONLY || f == libc::O_RDWR {
        return Ok(());
    }
    Err(zx::Status::INVALID_ARGS)
}

#[test]
fn test_prevalidate_flags() {
    assert!(prevalidate_flags(libc::O_PATH).is_ok());
    assert!(prevalidate_flags(libc::O_RDONLY).is_ok());
    assert!(prevalidate_flags(libc::O_WRONLY).is_ok());
    assert!(prevalidate_flags(libc::O_RDWR).is_ok());

}

/// Vfs contains filesystem global state and outlives all Vnodes that it
/// services. It fundamentally handles filesystem global concerns such as path
/// walking through mounts, moves and links, watchers, and so on.
pub trait Vfs {
    fn open(
        &self,
        _vn: &Arc<Vnode>,
        _path: std::path::PathBuf,
        _flags: u32,
        _mode: u32,
    ) -> Result<(Arc<Vnode>, std::path::PathBuf), zx::Status> {
        // TODO(raggi): locking


        Err(zx::Status::NOT_SUPPORTED)
    }

    fn register_connection(&self, c: Connection, handle: &tokio_core::reactor::Handle) {
        handle.spawn(c.map_err(
            |e| eprintln!("fuchsia-vfs: connection error {:?}", e),
        ))
    }
}

/// Vnode represents a single addressable node in the filesystem (that may be
/// addressable via more than one path). It may have file, directory, mount or
/// device semantics.
pub trait Vnode {
    fn close(&self) -> zx::Status {
        zx::Status::OK
    }

    /// If the Vnode should be served as a regular FDIO connection, consume the
    /// flags as required and return the channel. A Connection will be
    /// constructed and FDIO messages dispatched to this Vnode. Otherwise,
    /// consume the channel and return None.
    fn should_serve(&self, chan: tokio_fuchsia::Channel, _flags: u32, _handle: &tokio_core::reactor::Handle) -> Option<tokio_fuchsia::Channel> {
        Some(chan)
    }
}

/// Connection represents a single client connection to a Vnode within a Vfs. It
/// contains applicable IO state such as current position, as well as the channel
/// on which the IO is served.
pub struct Connection {
    vfs: Arc<Vfs>,
    vn: Arc<Vnode>,
    chan: tokio_fuchsia::Channel,
    handle: tokio_core::reactor::Handle,
}

impl Connection {
    pub fn new(
        vfs: Arc<Vfs>,
        vn: Arc<Vnode>,
        chan: zx::Channel,
        handle: &tokio_core::reactor::Handle,
    ) -> Result<Connection, zx::Status> {
        let c = Connection {
            vfs: vfs,
            vn: vn,
            chan: tokio_fuchsia::Channel::from_channel(chan, handle)?,
            handle: handle.clone(),
        };

        Ok(c)
    }

    fn dispatch(&mut self, msg: &mut fdio::rio::Message) -> Result<(), zx::Status> {
        let pipelined = msg.arg() & fdio::fdio_sys::O_PIPELINE != 0;

        if let Err(e) = msg.validate() {
            println!("{:?} <- {:?} (INVALID {:?})", self.chan, msg, e);
            // if the request is pipelined, just drop the reply channel and all is well
            if !pipelined {
                self.reply_status(&self.chan, zx::Status::INVALID_ARGS)?;
                // TODO(raggi): return ok here? need to define what dispatch errors really mean
                return Err(zx::Status::INVALID_ARGS.into());
            }
        }

        println!("{:?} <- {:?}", self.chan, msg);

        match msg.op() {
            fdio::fdio_sys::ZXRIO_OPEN => {
                let chan = tokio_fuchsia::Channel::from_channel(
                    zx::Channel::from(
                        msg.take_handle(0).expect("vfs: handle disappeared"),
                    ),
                    &self.handle,
                )?;

                // TODO(raggi): enforce O_ADMIN
                if msg.datalen() < 1 || msg.datalen() > PATH_MAX as u32 {
                    if !pipelined {
                        self.reply_status(&self.chan, zx::Status::INVALID_ARGS)?;
                    }
                    // TODO(raggi): return ok here? need to define what dispatch errors really mean
                    return Err(zx::Status::INVALID_ARGS.into());
                }

                let path = std::path::PathBuf::from(std::ffi::OsStr::from_bytes(msg.data()));

                // TODO(raggi): verify if the protocol mistreatment of args signage is intentionally unchecked here:
                self.open(chan, path, msg.arg(), msg.mode())
            }
            // ZXRIO_STAT => self.stat(msg, chan, handle),
            // ZXRIO_CLOSE => self.close(msg, chan, handle),
            _ => {
                self.reply_status(
                    &self.chan,
                    zx::Status::NOT_SUPPORTED,
                )
            }
        }
    }

    fn open(
        &self,
        chan: tokio_fuchsia::Channel,
        path: std::path::PathBuf,
        flags: i32,
        mode: u32,
    ) -> Result<(), zx::Status> {
        let pipeline = flags & fdio::fdio_sys::O_PIPELINE != 0;
        let open_flags: u32 = (flags & !fdio::fdio_sys::O_PIPELINE) as u32;

        let mut status = zx::Status::OK;
        let mut proto = fdio::fdio_sys::FDIO_PROTOCOL_REMOTE;
        let mut handles: Vec<zx::Handle> = vec![];

        match self.vfs.open(&self.vn, path, open_flags, mode) {
            Ok((vn, _path)) => {
                // TODO(raggi): get_handles (maybe call it get_extra?)

                // protocols that return handles on open can't be pipelined.
                if pipeline && handles.len() > 0 {
                    vn.close();
                    return Err(std::io::ErrorKind::InvalidInput.into());
                }

                if status != zx::Status::OK {
                    return Err(std::io::ErrorKind::InvalidInput.into());
                }

                if let Some(chan) = vn.should_serve(chan, open_flags, &self.handle) {

                    if !pipeline {
                        self.reply_object(&chan, status, proto, &[], &mut handles)?;
                    }

                    self.vfs.register_connection(Connection{vfs: self.vfs.clone(), vn, chan, handle: self.handle.clone()}, &self.handle)
                }
                // if should_serve consumed the channel, it must also handle the reply
                return Ok(())
            }
            Err(e) => {
                proto = 0;
                status = e;
                eprintln!("vfs: open error: {:?}", e);

                if !pipeline {
                    self.reply_object(&chan, status, proto, &[], &mut handles)?;
                }
                return Ok(())
            }
        }
    }

    fn reply_object(
        &self,
        chan: &tokio_fuchsia::Channel,
        status: zx::Status,
        type_: u32,
        extra: &[u8],
        handles: &mut Vec<zx::Handle>,
    ) -> Result<(), zx::Status> {
        println!("{:?} -> {:?}", &chan, status);
        fdio::rio::write_object(chan, status, type_, extra, handles)
    }

    fn reply_status(
        &self,
        chan: &tokio_fuchsia::Channel,
        status: zx::Status,
    ) -> Result<(), zx::Status> {
        self.reply_object(chan, status, 0, &[], &mut vec![])
    }
}

impl Future for Connection {
    type Item = ();
    type Error = io::Error;

    fn poll(&mut self) -> futures::Poll<Self::Item, Self::Error> {
        let mut buf = zx::MessageBuf::new();
        buf.ensure_capacity_bytes(fdio::fdio_sys::ZXRIO_MSG_SZ);
        loop {
            try_nb!(self.chan.recv_from(&mut buf));
            let mut msg = buf.into();
            // Note: ignores errors, as they are sent on the protocol
            let _ = self.dispatch(&mut msg);
            buf = msg.into();
            buf.clear();
        }
    }
}
