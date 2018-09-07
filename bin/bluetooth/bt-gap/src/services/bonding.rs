// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use crate::host_dispatcher::HostDispatcher;
use fidl;
use fidl::endpoints::RequestStream;
use fidl_fuchsia_bluetooth;
use fidl_fuchsia_bluetooth_control::{BondingRequest, BondingRequestStream};
use fuchsia_async;
use fuchsia_bluetooth::bt_fidl_status;
use fuchsia_syslog::{fx_log, fx_log_info};
use futures::{TryFutureExt, TryStreamExt};

pub async fn start_bonding_service(
    mut hd: HostDispatcher, channel: fuchsia_async::Channel,
) -> fidl::Result<()> {

    let stream = BondingRequestStream::from_channel(channel);
    hd.set_bonding_listener(Some(stream.control_handle()));
    await!(stream.try_for_each(|event| handler(hd.clone(), event)))
}

pub async fn handler(mut hd: HostDispatcher, event: BondingRequest) -> fidl::Result<()> {
    let BondingRequest::AddBondedDevices { local_id, bonds, responder } = event;
    fx_log_info!("Add Bonded devices for {:?}", local_id);
    await!(hd.get_active_adapter().map_ok(move |host_device| {
        if let Some(ref host_device) = host_device {
            host_device.read().restore_bonds(bonds);
        }
        responder.send(&mut bt_fidl_status!()).unwrap()
    }))
}
