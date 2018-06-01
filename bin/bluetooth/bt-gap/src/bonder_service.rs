// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use async;
use fidl::encoding2::OutOfLine;
use fidl_bluetooth;
use fidl_bluetooth_bonder::{Bonder, BonderImpl};
use futures::{future, Future, FutureExt, Never};
use futures::future::Either::{Left, Right};
use futures::future::ok as fok;
use futures::prelude::*;
use host_dispatcher::HostDispatcher;
use parking_lot::RwLock;
use std::sync::Arc;
use zx::Duration;
use async::TimeoutExt;

pub fn make_bonder_service(
    hd: Arc<RwLock<HostDispatcher>>, chan: async::Channel
) -> impl Future<Item = (), Error = Never> {
    BonderImpl {
        state: hd,
        on_open: |state, handle| {
           //w let wstate = state.write();
            //let mut hd = state.write();
            //hd.events = Some(handle.clone());
            fok(())
        },
        add_bonded_devices: |hd, local_id, bonds, res| {
            // TODO use local_id instead of the active adapter
            // initialization issues create a lock?
            HostDispatcher::get_active_adapter(hd.clone()).and_then(|host_device| {
                if let Some(ref host_device) = host_device {
                    host_device.read().bond(bonds);
                }
                fok(())
            }).recover(|e| eprintln!("error sending response: {:?}", e))
        },
    }.serve(chan)
        .recover(|e| eprintln!("error running service: {:?}", e))
}
