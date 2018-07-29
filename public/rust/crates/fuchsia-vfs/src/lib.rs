// Copyright 2017 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Fuchsia VFS Server Bindings

#![feature(arbitrary_self_types, futures_api, pin)]
#![deny(warnings)]

use fuchsia_zircon as zx;

use std::path::Path;
use std::sync::Arc;
use fuchsia_zircon::AsHandleRef;

mod mount;

pub mod vfs;
pub use crate::vfs::*;

pub fn mount(
    path: &Path,
    vfs: Arc<Vfs>,
    vn: Arc<Vnode>,
) -> Result<mount::Mount, zx::Status> {
    let (c1, c2) = zx::Channel::create()?;
    let m = mount::mount(path, c1)?;
    c2.signal_handle(
        zx::Signals::NONE,
        zx::Signals::USER_0,
    )?;
    let c = Connection::new(Arc::clone(&vfs), vn, c2)?;
    vfs.register_connection(c);
    Ok(m)
}

#[cfg(test)]
mod test {
    use super::*;
    use fuchsia_async as fasync;
    use futures::channel::oneshot;
    use futures::io;
    use std::fs;
    use std::thread;

    struct BasicFS {}

    impl Vfs for BasicFS {}

    struct BasicVnode {}

    impl Vnode for BasicVnode {}

    #[test]
    fn mount_basic() {
        let mut executor = fasync::Executor::new().unwrap();

        let bfs = Arc::new(BasicFS {});
        let bvn = Arc::new(BasicVnode {});

        let d = tempdir::TempDir::new("mount_basic").unwrap();

        let m = mount(&d.path(), bfs, bvn).expect("mount");

        let (tx, rx) = oneshot::channel::<io::Error>();

        let path = d.path().to_owned();

        thread::spawn(move || {
            let e = fs::OpenOptions::new()
                .read(true)
                .open(path)
                .expect_err("expected notsupported");
            tx.send(e).unwrap();
        });

        let e = executor.run_singlethreaded(rx).unwrap();

        assert_eq!(io::ErrorKind::Other, e.kind());

        std::mem::drop(m);
    }
}
