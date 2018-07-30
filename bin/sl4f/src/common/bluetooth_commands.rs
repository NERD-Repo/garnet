// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use crate::bt::error::Error as BTError;
use crate::common::bluetooth_facade::BluetoothFacade;
use crate::common::constants::*;
use crate::fasync::{TimeoutExt, Timer};
use crate::fidl_ble::{AdvertisingData, ScanFilter};
use crate::zx::prelude::*;
use failure::Error;
use parking_lot::RwLock;
use serde_json::{to_value, Value};
use std::sync::Arc;

use crate::common::bluetooth_facade::{BleAdvertiseResponse, BluetoothMethod};

// Takes a serde_json::Value and converts it to arguments required for
// a FIDL ble_advertise command
fn ble_advertise_to_fidl(
    adv_args_raw: Value,
) -> Result<(Option<AdvertisingData>, Option<u32>), Error> {
    let adv_data_raw = match adv_args_raw.get("advertising_data") {
        Some(adr) => Some(adr).unwrap().clone(),
        None => return Err(BTError::new("Advertising data missing.").into()),
    };

    let interval_raw = match adv_args_raw.get("interval_ms") {
        Some(ir) => Some(ir).unwrap().clone(),
        None => return Err(BTError::new("Interval_ms missing.").into()),
    };

    // Unpack the name for advertising data, as well as interval of advertising
    let name: Option<String> = adv_data_raw["name"].as_str().map(String::from);
    let interval: Option<u32> = interval_raw.as_u64().map(|i| i as u32);

    // TODO(aniramakri): Is there a better way to unpack the args into an AdvData
    // struct? Unfortunately, can't derive deserialize for AdvData
    let ad = Some(AdvertisingData {
        name: name,
        tx_power_level: None,
        appearance: None,
        service_uuids: None,
        service_data: None,
        manufacturer_specific_data: None,
        solicited_service_uuids: None,
        uris: None,
    });

    Ok((ad, interval))
}

// Takes a serde_json::Value and converts it to arguments required for a FIDL
// ble_scan command
fn ble_scan_to_fidl(
    scan_args_raw: Value,
) -> Result<(Option<ScanFilter>, Option<u64>, Option<u64>), Error> {
    let timeout_raw = match scan_args_raw.get("scan_time_ms") {
        Some(t) => Some(t).unwrap().clone(),
        None => return Err(BTError::new("Timeout_ms missing.").into()),
    };

    let scan_filter_raw = match scan_args_raw.get("filter") {
        Some(f) => Some(f).unwrap().clone(),
        None => return Err(BTError::new("Scan filter missing.").into()),
    };

    let scan_count_raw = match scan_args_raw.get("scan_count") {
        Some(c) => Some(c).unwrap().clone(),
        None => return Err(BTError::new("Scan count missing.").into()),
    };

    let timeout: Option<u64> = timeout_raw.as_u64();
    let name_substring: Option<String> =
        scan_filter_raw["name_substring"].as_str().map(String::from);
    let count: Option<u64> = scan_count_raw.as_u64();

    // For now, no scan profile, so default to empty ScanFilter
    let filter = Some(ScanFilter {
        service_uuids: None,
        service_data_uuids: None,
        manufacturer_identifier: None,
        connectable: None,
        name_substring: name_substring,
        max_path_loss: None,
    });

    Ok((filter, timeout, count))
}

// Takes a serde_json::Value and converts it to arguments required for a FIDL
// stop_advertising command. For stop advertise, no arguments are sent, rather
// uses current advertisement id (if it exists)
fn ble_stop_advertise_to_fidl(
    _stop_adv_args_raw: Value, bt_facade: Arc<RwLock<BluetoothFacade>>,
) -> Result<String, Error> {
    let adv_id = bt_facade.read().get_adv_id().clone();

    match adv_id.name {
        Some(aid) => Ok(aid.to_string()),
        None => Err(BTError::new("No advertisement id outstanding.").into()),
    }
}

// Takes ACTS method command and executes corresponding FIDL method
// Packages result into serde::Value
pub async fn ble_method_to_fidl(
    method_name: String, args: Value, bt_facade: Arc<RwLock<BluetoothFacade>>,
) -> Result<Value, Error> {
    match BluetoothMethod::from_str(method_name) {
        BluetoothMethod::BleAdvertise => {
            let (ad, interval) = match ble_advertise_to_fidl(args) {
                Ok((adv_data, intv)) => (adv_data, intv),
                Err(e) => return Err(e),
            };
            Ok(await!(start_adv_async(bt_facade.clone(), ad, interval))?)
        }
        BluetoothMethod::BleScan => {
            let (filter, timeout, count) = match ble_scan_to_fidl(args) {
                Ok((f, t, c)) => (f, t, c),
                Err(e) => return Err(e),
            };
            Ok(await!(start_scan_async(
                bt_facade.clone(),
                filter,
                timeout,
                count
            ))?)
        }
        BluetoothMethod::BleStopAdvertise => {
            let advertisement_id = match ble_stop_advertise_to_fidl(args, bt_facade.clone()) {
                Ok(aid) => aid,
                Err(e) => return Err(e),
            };
            Ok(await!(stop_adv_async(
                bt_facade.clone(),
                advertisement_id.clone()
            ))?)
        }
        _ => Err(BTError::new("Invalid BLE FIDL method").into()),
    }
}

async fn start_adv_async(
    bt_facade: Arc<RwLock<BluetoothFacade>>, ad: Option<AdvertisingData>, interval: Option<u32>,
) -> Result<Value, Error> {
    let mut bt_facade = bt_facade.write();
    if let Some(adv_id) = await!(bt_facade.start_adv(ad, interval))? {
        bt_facade.update_adv_id(Some(adv_id.clone()));
        let aid_response = BleAdvertiseResponse::new(Some(adv_id));
        return match to_value(aid_response) {
            Ok(val) => Ok(val),
            Err(e) => Err(e.into()),
        };
    };
    Err(BTError::new("Failed to start adv sync").into())
}

async fn stop_adv_async(
    bt_facade: Arc<RwLock<BluetoothFacade>>, adv_id: String,
) -> Result<Value, Error> {
    let facade = bt_facade.write();
    let res = await!(facade.stop_adv(adv_id))?;
    BluetoothFacade::cleanup_peripheral(bt_facade.clone());
    match to_value(res) {
        Ok(val) => Ok(val),
        Err(e) => Err(e.into()),
    }
}

// Synchronous wrapper for scanning
async fn start_scan_async(
    bt_facade: Arc<RwLock<BluetoothFacade>>, filter: Option<ScanFilter>, timeout: Option<u64>,
    count: Option<u64>,
) -> Result<Value, Error> {
    let timeout_ms = timeout.unwrap_or(DEFAULT_SCAN_TIMEOUT_MS).millis();
    // Create scanning future and listen on central events for scan
    await!(BluetoothFacade::start_scan(bt_facade.clone(), filter))?;

    // Future resolves when number of devices discovered is equal to count
    if count.is_none() {
        await!(Timer::new(timeout_ms.after_now()))
    } else {
        await!(
            BluetoothFacade::new_devices_found_future(bt_facade.clone(), count.unwrap())
                .on_timeout(timeout_ms.after_now(), || Ok(()))
        )?
    };

    // After futures resolve, grab set of devices discovered and stop the scan
    let devices = bt_facade.read().get_devices();

    // Grab the central proxy created
    let central = bt_facade
        .read()
        .get_central_proxy()
        .clone()
        .expect("No central proxy.");

    if let Err(e) = central.stop_scan() {
        Err(e.into())
    } else {
        match to_value(devices) {
            Ok(dev) => Ok(dev),
            Err(e) => Err(e.into()),
        }
    }
}
