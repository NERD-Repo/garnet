// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

/// THE NEW BT-GAP

use crate::host_device::{self, HostDevice};
use crate::services;
use crate::store::stash::Stash;
use crate::util;
use failure::Error;
use fidl;
use fidl::encoding::OutOfLine;
use fidl::endpoints::ServerEnd;
use fidl_fuchsia_bluetooth{self, Status};
use fidl_fuchsia_bluetooth_control::{
    AdapterInfo,
    ControlControlHandle,
    PairingDelegateMarker,
    PairingDelegateProxy,
    InputCapabilityType,
    OutputCapabilityType,
    RemoteDevice
};
use fidl_fuchsia_bluetooth_bredr::ProfileMarker;
use fidl_fuchsia_bluetooth_gatt::Server_Marker;
use fidl_fuchsia_bluetooth_host::{HostProxy, BondingData};
use fidl_fuchsia_bluetooth_le::{CentralMarker, PeripheralMarker};
use fuchsia_bluetooth::{
    self as bt,
    bt_fidl_status,
    error::Error as BTError,
    util::clone_host_info,
    util::clone_remote_device
};
use fuchsia_async::{self as fasync, TimeoutExt};
use fuchsia_syslog::{fx_log, fx_log_err, fx_log_info, fx_log_warn};
use fuchsia_vfs_watcher as vfs_watcher;
use fuchsia_vfs_watcher::{WatchEvent, WatchMessage};
use fuchsia_zircon as zx;
use fuchsia_zircon::Duration;
use futures::TryStreamExt;
use futures::{task::{LocalWaker, Waker}, Future, Poll, TryFutureExt};
use parking_lot::RwLock;
use slab::Slab;
use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::marker::Unpin;
use std::path::PathBuf;
use std::sync::{Arc, Weak};


/// The HostDispatcher acts as a proxy aggregating multiple HostAdapters
/// It appears as a Host to higher level systems, and is responsible for
/// routing commands to the appropriate HostAdapter

pub struct HostDispatcher {}

impl HostDispatcher {
    pub fn adapters(&self) -> fidl::Result<Vec<AdapterInfo>>;
    pub fn active_adapter(&self) -> Option<AdapterInfo>;
    pub fn set_active_adapter(&mut self, adapter_id: String) -> Status;

    pub fn remote_devices(&self) -> Vec<RemoteDevice>;

    pub async fn connect(&mut self, device_id: String) -> fidl::Result<Status>;
    pub async fn disconnect(&mut self, device_id: String) -> fidl::Result<Status>;
    pub async fn forget(&mut self, _device_id: String) -> fidl::Result<Status>; 
    pub async fn set_name(&mut self, name: Option<String>) -> fidl::Result<Status>;
    pub async fn start_discovery(&mut self) -> fidl::Result<(Status, Option<DiscoveryRequestToken>)>;
    pub async fn set_discoverable(&mut self) -> fidl::Result<(Status, Option<DiscoverableRequestToken>)>;

    pub fn set_pairing_delegate(&mut self, delegate: Option<PairingDelegateProxy>) -> bool;

    pub fn request_host_service(mut self, chan: fasync::Channel, service: HostService);

    pub fn set_io_capability(&mut self, input: InputCapabilityType, output: OutputCapabilityType);

    pub fn add_event_listener(&mut self, handle: Weak<ControlControlHandle>);

    pub fn store_bond(&mut self, bond: BondingData) -> Result<(),Error>;
}
