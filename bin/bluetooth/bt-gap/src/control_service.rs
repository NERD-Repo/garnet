// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use crate::fasync;
use crate::host_dispatcher::*;
use fidl::encoding2::OutOfLine;
use fidl::endpoints2::RequestStream;
use fidl::Error;
use fidl_fuchsia_bluetooth;
use fidl_fuchsia_bluetooth_control::{ControlRequest, ControlRequestStream};
use futures::prelude::*;
use parking_lot::RwLock;
use std::sync::Arc;

struct ControlServiceState {
    host: Arc<RwLock<HostDispatcher>>,
    discovery_token: Option<Arc<DiscoveryRequestToken>>,
    discoverable_token: Option<Arc<DiscoverableRequestToken>>,
}

/// Build the ControlImpl to interact with fidl messages
/// State is stored in the HostDispatcher object
pub async fn make_control_service(
    hd: Arc<RwLock<HostDispatcher>>, chan: fasync::Channel,
) -> Result<(), Error> {
    let mut requests = ControlRequestStream::from_channel(chan);

    // register listener to push events too
    hd.write().event_listeners.push(requests.control_handle());

    let state = Arc::new(RwLock::new(ControlServiceState {
        host: hd,
        discovery_token: None,
        discoverable_token: None,
    }));

    // TODO capture handle for event_listener

    while let Some(res) = await!(requests.next()) {
        match res? {
            ControlRequest::Connect {
                device_id: _,
                responder,
            } => responder.send(&mut bt_fidl_status!(NotSupported))?,
            ControlRequest::IsBluetoothAvailable { responder } => {
                let rstate = state.read();
                let mut hd = rstate.host.write();
                let is_available = hd.get_active_adapter_info().is_some();
                responder.send(is_available)?
            }
            ControlRequest::SetPairingDelegate {
                delegate,
                responder,
            } => {
                let mut wstate = state.write();
                if let Some(delegate) = delegate {
                    if let Ok(proxy) = delegate.into_proxy() {
                        wstate.host.write().pairing_delegate = Some(proxy);
                        responder.send(true)?
                    } else {
                        responder.send(false)?
                    }
                } else {
                    wstate.host.write().pairing_delegate = None;
                    responder.send(true)?
                }
            }
            ControlRequest::GetAdapters { responder } => {
                let wstate = state.write();
                let mut hd = wstate.host.clone();
                let mut adapters = await!(HostDispatcher::get_adapters(&mut hd))?;
                // work around ICE. TODO just return iter_mut of adapters
                let mut a = vec![];
                for x in adapters {
                    a.push(x);
                }
                responder.send(Some(&mut a.iter_mut()))?
            }
            ControlRequest::SetActiveAdapter {
                identifier,
                responder,
            } => {
                let wstate = state.write();
                let mut success = wstate.host.write().set_active_adapter(identifier.clone());
                responder.send(&mut success)?
            }
            ControlRequest::RequestDiscovery {
                discovery,
                responder,
            } => {
                if discovery {
                    let stateref = state.clone();
                    let (mut resp, token) =
                        await!(HostDispatcher::start_discovery(state.read().host.clone()))?;
                    stateref.write().discovery_token = token;
                    responder.send(&mut resp)?
                } else {
                    state.write().discovery_token = None;
                    responder.send(&mut bt_fidl_status!())?
                }
            }
            ControlRequest::GetKnownRemoteDevices { responder: _ } => (),
            ControlRequest::GetActiveAdapterInfo { responder } => {
                let wstate = state.write();
                let mut hd = wstate.host.write();
                let mut adap = hd.get_active_adapter_info();
                responder.send(adap.as_mut().map(OutOfLine))?
            }
            ControlRequest::SetName { name, responder } => {
                let wstate = state.write();
                let mut _resp = await!(HostDispatcher::set_name(wstate.host.clone(), name))?;
                responder.send(&mut bt_fidl_status!())?
            }
            ControlRequest::SetDiscoverable {
                discoverable,
                responder,
            } => {
                if discoverable {
                    let stateref = state.clone();
                    let (mut resp, token) =
                        await!(HostDispatcher::set_discoverable(state.read().host.clone()))?;
                    stateref.write().discoverable_token = token;
                    responder.send(&mut resp)?
                } else {
                    state.write().discoverable_token = None;
                    responder.send(&mut bt_fidl_status!())?;
                }
            }
            ControlRequest::Disconnect {
                device_id: _,
                responder,
            } => responder.send(&mut bt_fidl_status!(NotSupported))?,
            ControlRequest::Forget {
                device_id: _,
                responder,
            } => responder.send(&mut bt_fidl_status!(NotSupported))?,
            ControlRequest::SetIoCapabilities {
                input: _,
                output: _,
                control_handle: _,
            } => (),
        };
    }

    Ok(())
}
