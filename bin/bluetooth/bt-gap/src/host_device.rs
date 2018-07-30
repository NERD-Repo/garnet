// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use crate::host_dispatcher::HostDispatcher;
use crate::util::clone_host_state;
use fidl;
use fidl_fuchsia_bluetooth::Status;
use fidl_fuchsia_bluetooth_control::AdapterInfo;
use fidl_fuchsia_bluetooth_host::{HostEvent, HostProxy};
use futures::StreamExt;
use parking_lot::RwLock;
use std::path::PathBuf;
use std::sync::Arc;

pub struct HostDevice {
    pub path: PathBuf,

    host: HostProxy,
    info: AdapterInfo,
}

impl HostDevice {
    pub fn new(path: PathBuf, host: HostProxy, info: AdapterInfo) -> Self {
        HostDevice { path, host, info }
    }

    pub fn get_host(&self) -> &HostProxy {
        &self.host
    }

    pub fn get_info(&self) -> &AdapterInfo {
        &self.info
    }

    pub async fn set_name(&self, mut name: String) -> Result<Status, fidl::Error> {
        Ok(await!(self.host.set_local_name(&mut name))?)
    }

    pub async fn start_discovery(&mut self) -> Result<Status, fidl::Error> {
        Ok(await!(self.host.start_discovery())?)
    }

    pub fn close(&self) -> Result<(), fidl::Error> {
        self.host.close()
    }

    pub async fn stop_discovery(&self) -> Result<Status, fidl::Error> {
        Ok(await!(self.host.stop_discovery())?)
    }

    pub async fn set_discoverable(&mut self, discoverable: bool) -> Result<Status, fidl::Error> {
        Ok(await!(self.host.set_discoverable(discoverable))?)
    }
}

pub async fn run(
    hd: Arc<RwLock<HostDispatcher>>, host: Arc<RwLock<HostDevice>>,
) -> Result<(), fidl::Error> {
    make_clones!(host => host_stream, host);
    let mut stream = host_stream.read().host.take_event_stream();
    while let Some(evt) = await!(stream.next()) {
        match evt? {
            HostEvent::OnAdapterStateChanged { ref state } => {
                host.write().info.state = Some(Box::new(clone_host_state(&state)));
            }
            HostEvent::OnDeviceUpdated { mut device } => {
                for listener in hd.read().event_listeners.iter() {
                    let _res = listener
                        .send_on_device_updated(&mut device)
                        .map_err(|e| error!("Failed to send device updated event: {:?}", e));
                }
            }
            HostEvent::OnDeviceRemoved { identifier } => {
                for listener in hd.read().event_listeners.iter() {
                    let _res = listener
                        .send_on_device_removed(&identifier)
                        .map_err(|e| error!("Failed to send device removed event: {:?}", e));
                }
            }
            HostEvent::OnNewBondingData { .. } => {
                unimplemented!("not yet");
            }
        }
    }
    Ok(())
}
