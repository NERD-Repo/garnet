// Copyright 2017 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Fuchsia VFS Server Bindings

extern crate bytes;
extern crate fdio;
extern crate fuchsia_zircon as zx;
extern crate futures;
extern crate libc;
#[macro_use]
extern crate tokio_core;
extern crate tokio_fuchsia;

use std::path::Path;
use std::sync::Arc;
use zx::AsHandleRef;

mod mount;

pub mod vfs;
pub use vfs::*;

pub fn mount(
    path: &Path,
    vfs: Arc<Vfs>,
    vn: Arc<Vnode>,
    handle: &tokio_core::reactor::Handle,
) -> Result<mount::Mount, zx::Status> {
    let (c1, c2) = zx::Channel::create()?;
    let m = mount::mount(path, c1)?;
    c2.signal_handle(
        zx::Signals::NONE,
        zx::Signals::USER_0,
    )?;
    let c = Connection::new(Arc::clone(&vfs), vn, c2, handle)?;
    vfs.register_connection(c, handle);
    Ok(m)
}

#[cfg(test)]
mod test {
    use super::*;
    use std::os::unix::ffi::OsStrExt;
    use std::os::raw::c_char;

    extern crate tempdir;

    struct BasicFS {}

    impl Vfs for BasicFS {}

    struct BasicVnode {}

    impl Vnode for BasicVnode {}

    #[test]
    fn mount_basic() {
        let mut core = tokio_core::reactor::Core::new().unwrap();

        let bfs = Arc::new(BasicFS {});
        let bvn = Arc::new(BasicVnode {});

        let d = tempdir::TempDir::new("mount_basic").unwrap();

        let m = mount(&d.path(), bfs, bvn, &core.handle()).expect("mount");

        let (tx, rx) = futures::sync::oneshot::channel::<std::io::Error>();

        let path = d.path().to_owned();

        std::thread::spawn(move || {
            eprintln!("thread start");
            eprintln!("about to open");

            let e = std::fs::OpenOptions::new()
                .read(true)
                .open(path)
                .expect_err("expected notsupported");

            eprintln!("open done");
            tx.send(e).unwrap();
            eprintln!("thread done");
        });

            // XXX(raggi): deterministic deadlock:
            std::thread::sleep(std::time::Duration::from_millis(100));

        eprintln!("about to run receive future");
        let e = core.run(rx).unwrap();
        eprintln!("receive future complete");

        assert_eq!(std::io::ErrorKind::Other, e.kind());

        eprintln!("about to drop (unmount)");
        std::mem::drop(m);
        eprintln!("drop (unmount) done");
    }

    // hellofs implements a filesystem that exposes:
    //  ./
    //    hello/
    //          world
    //               `hello world`
    struct HelloFS {}

    impl Vfs for HelloFS {
        fn open(&self, vn: &Arc<Vnode>, path: std::path::PathBuf, flags: u32, mode: u32) -> Result<(Arc<Vnode>, std::path::PathBuf), zx::Status> {
            Err(zx::Status::NOT_FOUND)
        }
    }

    struct HelloDir {}

    impl Vnode for HelloDir {}

    struct HelloFile {}

    impl Vnode for HelloFile {}
}
