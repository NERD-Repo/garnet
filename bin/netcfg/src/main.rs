// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#![feature(async_await, await_macro, futures_api)]
#![deny(warnings)]

use failure::{Error, ResultExt};
use fidl_fuchsia_devicesettings::{DeviceSettingsManagerMarker};
use fidl_fuchsia_netstack::{NetstackMarker, NetInterface, NetstackEvent, INTERFACE_FEATURE_SYNTH, INTERFACE_FEATURE_LOOPBACK};
use fuchsia_app as app;
use fuchsia_async as fasync;
use futures::StreamExt;
use serde_derive::Deserialize;
use std::fs;
use std::io::Read;

mod device_id;

const DEFAULT_CONFIG_FILE: &str = "/pkg/data/default.json";

#[derive(Debug, Deserialize)]
pub struct Config {
    pub device_name: Option<String>,
}

fn parse_config(config: String) -> Result<Config, Error> {
    serde_json::from_str(&config).map_err(Into::into)
}

fn is_physical(n: &NetInterface) -> bool {
    (n.features & (INTERFACE_FEATURE_SYNTH | INTERFACE_FEATURE_LOOPBACK)) == 0
}

fn derive_device_name(interfaces: Vec<NetInterface>) -> Option<String> {
    interfaces.iter()
        .filter(|iface| is_physical(iface))
        .min_by(|a, b| a.id.cmp(&b.id))
        .map(|iface| device_id::device_id(&iface.hwaddr))
}

// Workaround for https://fuchsia.atlassian.net/browse/TC-141
fn read_to_string(s: &str) -> Result<String, Error> {
    let mut f = fs::File::open(s).context("Failed to read file")?;
    let mut out = String::new();
    f.read_to_string(&mut out)?;
    Ok(out)
}

static DEVICE_NAME_KEY: &str = "DeviceName";

fn main() -> Result<(), Error> {
    println!("netcfg: started");
    let default_config = parse_config(read_to_string(DEFAULT_CONFIG_FILE)?)?;
    let mut executor = fasync::Executor::new().context("error creating event loop")?;
    let netstack = app::client::connect_to_service::<NetstackMarker>().context("failed to connect to netstack")?;
    let device_settings_manager = app::client::connect_to_service::<DeviceSettingsManagerMarker>()
        .context("failed to connect to device settings manager")?;

    let fut = async move || {
        if let Some(name) = default_config.device_name {
            await!(device_settings_manager.set_string(DEVICE_NAME_KEY, &name));
            return Ok(());
        }

        let mut events = netstack.take_event_stream();
        while let Some(e) = await!(events.next()) {
            match e? {
                NetstackEvent::OnInterfacesChanged { interfaces: is } => {
                    if let Some(name) = derive_device_name(is) {
                        await!(device_settings_manager.set_string(DEVICE_NAME_KEY, &name))?;
                    }
                    return Ok(())
                }
            }
        }
        Ok(())
    };

    executor.run_singlethreaded(fut())
}
