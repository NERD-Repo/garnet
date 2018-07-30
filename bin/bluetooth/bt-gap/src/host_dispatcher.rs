// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use crate::bt::util::clone_host_info;
use crate::fasync::futures::Future;
use crate::fasync::futures::Poll;
use crate::fasync::{self, TimeoutExt};
use crate::futures::task::Waker;
use crate::host_device;
use crate::host_device::HostDevice;
use crate::util;
use crate::zx::Duration;
use failure::Error;
use fidl::encoding2::OutOfLine;
use fidl_fuchsia_bluetooth::Status;
use fidl_fuchsia_bluetooth_control::AdapterInfo;
use fidl_fuchsia_bluetooth_control::{ControlControlHandle, PairingDelegateProxy};
use fidl_fuchsia_bluetooth_host::HostProxy;
use fuchsia_bluetooth as bt;
use fuchsia_vfs_watcher as vfs_watcher;
use futures::StreamExt;
use parking_lot::RwLock;
use slab::Slab;
use std::collections::HashMap;
use std::fs::File;
use std::marker::Unpin;
use std::mem::PinMut;
use std::path::PathBuf;
use std::sync::{Arc, Weak};

pub static HOST_INIT_TIMEOUT: u64 = 5; // Seconds

static BT_HOST_DIR: &'static str = "/dev/class/bt-host";
static DEFAULT_NAME: &'static str = "fuchsia";

pub struct DiscoveryRequestToken {
    adap: Weak<RwLock<HostDevice>>,
}

impl Drop for DiscoveryRequestToken {
    fn drop(&mut self) {
        if let Some(host) = self.adap.upgrade() {
            let mut host = host.write();
            host.stop_discovery();
        }
    }
}

pub struct DiscoverableRequestToken {
    adap: Weak<RwLock<HostDevice>>,
}

impl Drop for DiscoverableRequestToken {
    fn drop(&mut self) {
        if let Some(host) = self.adap.upgrade() {
            let mut host = host.write();
            host.set_discoverable(false);
        }
    }
}

pub struct HostDispatcher {
    host_devices: HashMap<String, Arc<RwLock<HostDevice>>>,
    active_id: Option<String>,

    // GAP state
    name: String,
    discovery: Option<Weak<DiscoveryRequestToken>>,
    discoverable: Option<Weak<DiscoverableRequestToken>>,

    pub pairing_delegate: Option<PairingDelegateProxy>,
    pub event_listeners: Vec<ControlControlHandle>,

    // Pending requests to obtain a Host.
    host_requests: Slab<Waker>,
}

impl HostDispatcher {
    pub fn new() -> HostDispatcher {
        HostDispatcher {
            active_id: None,
            host_devices: HashMap::new(),
            name: DEFAULT_NAME.to_string(),
            discovery: None,
            discoverable: None,
            pairing_delegate: None,
            event_listeners: vec![],
            host_requests: Slab::new(),
        }
    }

    pub async fn set_name(
        hd: Arc<RwLock<HostDispatcher>>, name: Option<String>,
    ) -> Result<Status, fidl::Error> {
        hd.write().name = match name {
            Some(name) => name,
            None => DEFAULT_NAME.to_string(),
        };
        if let Some(adapter) = await!(HostDispatcher::get_active_adapter(hd.clone()))? {
            let adapter = adapter.write();
            return Ok(await!(adapter.set_name(hd.read().name.clone()))?);
        };
        Ok(bt_fidl_status!(BluetoothNotAvailable, "No Adapter found"))
    }

    /// Return the active id. If the ID is current not set,
    /// this fn will make the first ID in it's host_devices active
    fn get_active_id(&mut self) -> Option<String> {
        match self.active_id {
            None => {
                let id = match self.host_devices.keys().next() {
                    None => {
                        return None;
                    }
                    Some(id) => id.clone(),
                };
                self.set_active_id(Some(id));
                self.active_id.clone()
            }
            ref id => id.clone(),
        }
    }

    pub async fn start_discovery(
        hd: Arc<RwLock<HostDispatcher>>,
    ) -> Result<
        (
            fidl_fuchsia_bluetooth::Status,
            Option<Arc<DiscoveryRequestToken>>,
        ),
        fidl::Error,
    > {
        let strong_current_token = match hd.read().discovery {
            Some(ref token) => token.upgrade(),
            None => None,
        };
        if let Some(token) = strong_current_token {
            return Ok((bt_fidl_status!(), Some(Arc::clone(&token))));
        }

        if let Some(adapter) = await!(HostDispatcher::get_active_adapter(hd.clone()))? {
            let weak_adapter = Arc::downgrade(&adapter);
            let mut adapter = adapter.write();
            let resp = await!(adapter.start_discovery())?;
            match resp.error {
                Some(_) => return Ok((resp, None)),
                None => {
                    let token = Arc::new(DiscoveryRequestToken { adap: weak_adapter });
                    hd.write().discovery = Some(Arc::downgrade(&token));
                    return Ok((resp, Some(token)));
                }
            }
        }
        Ok((
            bt_fidl_status!(BluetoothNotAvailable, "No Adapter found"),
            None,
        ))
    }

    pub async fn set_discoverable(
        hd: Arc<RwLock<HostDispatcher>>,
    ) -> Result<
        (
            fidl_fuchsia_bluetooth::Status,
            Option<Arc<DiscoverableRequestToken>>,
        ),
        fidl::Error,
    > {
        let strong_current_token = match hd.read().discoverable {
            Some(ref token) => token.upgrade(),
            None => None,
        };

        if let Some(token) = strong_current_token {
            return Ok((bt_fidl_status!(), Some(Arc::clone(&token))));
        }

        if let Some(adapter) = await!(HostDispatcher::get_active_adapter(hd.clone()))? {
            let weak_adapter = Arc::downgrade(&adapter);
            let mut adapter = adapter.write();
            let resp = await!(adapter.set_discoverable(true))?;
            match resp.error {
                Some(_) => return Ok((resp, None)),
                None => {
                    let token = Arc::new(DiscoverableRequestToken { adap: weak_adapter });
                    hd.write().discoverable = Some(Arc::downgrade(&token));
                    return Ok((resp, Some(token)));
                }
            }
        }
        Ok((
            bt_fidl_status!(BluetoothNotAvailable, "No Adapter found"),
            None,
        ))
    }

    pub fn set_active_adapter(&mut self, adapter_id: String) -> fidl_fuchsia_bluetooth::Status {
        if let Some(ref id) = self.active_id {
            if *id == adapter_id {
                return bt_fidl_status!(Already, "Adapter already active");
            }

            // Shut down the previously active host.
            let _ = self.host_devices[id].write().close();
        }

        if self.host_devices.contains_key(&adapter_id) {
            self.set_active_id(Some(adapter_id));
            bt_fidl_status!()
        } else {
            bt_fidl_status!(NotFound, "Attempting to activate an unknown adapter")
        }
    }

    pub fn get_active_adapter_info(&mut self) -> Option<AdapterInfo> {
        match self.get_active_id() {
            Some(ref id) => {
                // Id must always be valid
                let host = self.host_devices.get(id).unwrap().read();
                Some(util::clone_host_info(host.get_info()))
            }
            None => None,
        }
    }

    pub async fn get_active_adapter(
        hd: Arc<RwLock<HostDispatcher>>,
    ) -> Result<Option<Arc<RwLock<HostDevice>>>, fidl::Error> {
        let hd = await!(OnAdaptersFound::new(hd.clone()))?;
        let mut hd = hd.write();
        Ok(match hd.get_active_id() {
            Some(ref id) => Some(hd.host_devices.get(id).unwrap().clone()),
            None => None,
        })
    }

    pub async fn get_adapters(
        hd: &mut Arc<RwLock<HostDispatcher>>,
    ) -> Result<Vec<AdapterInfo>, fidl::Error> {
        let hd = await!(OnAdaptersFound::new(hd.clone()))?;
        let mut result = vec![];
        for host in hd.read().host_devices.values() {
            let host = host.read();
            result.push(util::clone_host_info(host.get_info()));
        }
        Ok(result)
    }

    // Resolves all pending OnAdapterFuture's. Called when we leave the init period (by seeing the
    // first host device or when the init timer expires).
    fn resolve_host_requests(&mut self) {
        for waker in &self.host_requests {
            waker.1.wake();
        }
    }

    fn add_host(&mut self, id: String, host: Arc<RwLock<HostDevice>>) {
        self.host_devices.insert(id, host);
    }

    /// Updates the active adapter and sends a FIDL event.
    fn set_active_id(&mut self, id: Option<String>) {
        info!("New active adapter: {:?}", id);
        self.active_id = id;
        if let Some(ref mut adapter_info) = self.get_active_adapter_info() {
            for events in self.event_listeners.iter() {
                let _res = events.send_on_active_adapter_changed(Some(OutOfLine(adapter_info)));
            }
        }
    }
}

/// A future that completes when at least one adapter is available.
#[must_use = "futures do nothing unless polled"]
struct OnAdaptersFound {
    hd: Arc<RwLock<HostDispatcher>>,
    waker_key: Option<usize>,
}

impl OnAdaptersFound {
    // Constructs an OnAdaptersFound that completes at the latest after HOST_INIT_TIMEOUT seconds.
    async fn new(
        hd: Arc<RwLock<HostDispatcher>>,
    ) -> Result<Arc<RwLock<HostDispatcher>>, fidl::Error> {
        Ok(await!(
            OnAdaptersFound {
                hd: hd.clone(),
                waker_key: None,
            }.on_timeout(
                Duration::from_seconds(HOST_INIT_TIMEOUT).after_now(),
                move || {
                    {
                        let hd = hd.write();
                        if hd.host_devices.len() == 0 {
                            info!("No bt-host devices found");
                            //hd.resolve_host_requests();
                        }
                    }
                    Ok(hd)
                }
            )
        )?)
    }

    fn remove_waker(&mut self) {
        if let Some(key) = self.waker_key {
            self.hd.write().host_requests.remove(key);
        }
        self.waker_key = None;
    }
}

impl Drop for OnAdaptersFound {
    fn drop(&mut self) {
        self.remove_waker()
    }
}

impl Unpin for OnAdaptersFound {}

impl Future for OnAdaptersFound {
    type Output = Result<Arc<RwLock<HostDispatcher>>, fidl::Error>;

    fn poll(mut self: PinMut<Self>, ctx: &mut futures::task::Context) -> Poll<Self::Output> {
        if self.hd.read().host_devices.len() == 0 {
            let hd = self.hd.clone();
            if self.waker_key.is_none() {
                self.waker_key = Some(hd.write().host_requests.insert(ctx.waker().clone()));
            }
            Poll::Pending
        } else {
            self.remove_waker();
            Poll::Ready(Ok(self.hd.clone()))
        }
    }
}

/// Adds an adapter to the host dispatcher. Called by the watch_hosts device watcher
async fn add_adapter(hd: Arc<RwLock<HostDispatcher>>, host_path: PathBuf) -> Result<(), Error> {
    info!("Adding Adapter: {:?}", host_path);
    let file = File::open(host_path.clone())?;
    let handle = bt::host::open_host_channel(&file).unwrap();
    let host = HostProxy::new(fasync::Channel::from_channel(handle.into()).unwrap());
    let adapter_info = await!(host.get_info())?;
    await!(host.set_connectable(true));
    let id = adapter_info.identifier.clone();
    let host_device = Arc::new(RwLock::new(HostDevice::new(host_path, host, adapter_info)));
    hd.write().add_host(id, host_device.clone());
    for listener in hd.read().event_listeners.iter() {
        let _res =
            listener.send_on_adapter_updated(&mut clone_host_info(host_device.read().get_info()));
    }
    info!("Host added: {:?}", host_device.read().get_info().identifier);
    hd.write().resolve_host_requests();
    Ok(await!(host_device::run(hd.clone(), host_device.clone()))?)
}

pub async fn rm_adapter(hd: Arc<RwLock<HostDispatcher>>, host_path: PathBuf) -> Result<(), Error> {
    info!("Host removed: {:?}", host_path);
    let mut hd = hd.write();
    let active_id = hd.active_id.clone();

    // Get the host IDs that match |host_path|.
    let ids: Vec<String> = hd
        .host_devices
        .iter()
        .filter(|(_, ref host)| host.read().path == host_path)
        .map(|(k, _)| k.clone())
        .collect();
    for id in &ids {
        hd.host_devices.remove(id);
    }

    // Reset the active ID if it got removed.
    if let Some(active_id) = active_id {
        if ids.contains(&active_id) {
            hd.active_id = None;
        }
    }

    // Try to assign a new active adapter. This may send an "OnActiveAdapterChanged" event.
    if hd.active_id.is_none() {
        let _ = hd.get_active_id();
    }

    Ok(())
}

pub async fn watch_hosts(hd: Arc<RwLock<HostDispatcher>>) -> Result<(), Error> {
    let file = File::open(&BT_HOST_DIR)?;
    let mut watcher = vfs_watcher::Watcher::new(&file)?;

    while let Some(evt) = await!(watcher.next()) {
        let evt = evt?;
        let path = PathBuf::from(format!(
            "{}/{}",
            BT_HOST_DIR,
            evt.filename.to_string_lossy()
        ));
        match evt.event {
            vfs_watcher::WatchEvent::EXISTING | vfs_watcher::WatchEvent::ADD_FILE => {
                info!("Adding device from {:?}", path);
                await!(add_adapter(hd.clone(), path))?
            }
            vfs_watcher::WatchEvent::REMOVE_FILE => {
                info!("Removing device from {:?}", path);
                await!(rm_adapter(hd.clone(), path))?
            }
            vfs_watcher::WatchEvent::IDLE => {
                debug!("HostDispatcher is IDLE");
                ()
            }
            e => {
                warn!("Unrecognized host watch event: {:?}", e);
                ()
            }
        }
    }
    Ok(())
}
