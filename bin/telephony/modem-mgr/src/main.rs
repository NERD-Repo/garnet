// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! System service for managing cellular modems

#![feature(async_await, await_macro, futures_api, arbitrary_self_types, pin)]
//#![deny(warnings)]
//#![deny(missing_docs)]
//
//

use fuchsia_app::{server::ServicesServer, client::Launcher};
use fidl_fuchsia_telephony_qmi::{QmiModemMarker, QmiModemProxy};
use fdio::{fdio_sys, ioctl_raw, make_ioctl};
use std::os::unix::io::AsRawFd;
use std::os::raw;
use fuchsia_zircon as zx;
use fuchsia_async as fasync;
use failure::{Error, ResultExt};
use std::path::{Path, PathBuf};
use fuchsia_vfs_watcher::{Watcher, WatchEvent};
use futures::{Future, Stream, future, TryStreamExt, TryFutureExt};
use std::fs::File;
use fuchsia_syslog::{self as syslog, macros::*};
use std::io;

const QMI_TRANSPORT: &str = "/dev/class/qmi-transport";

pub fn connect_qmi_transport_device(device: &File) -> Result<zx::Channel, zx::Status> {
    let mut handle: zx::sys::zx_handle_t = zx::sys::ZX_HANDLE_INVALID;

    // This call is safe because the callee does not retain any data from the call, and the return
    // value ensures that the handle is a valid handle to a zx::channel.
    unsafe {
        match ioctl_raw(
            device.as_raw_fd(),
            IOCTL_QMI_GET_CHANNEL,
            ::std::ptr::null(),
            0,
            &mut handle as *mut _ as *mut raw::c_void,
            ::std::mem::size_of::<zx::sys::zx_handle_t>(),
        ) as i32
        {
            e if e < 0 => Err(zx::Status::from_raw(e)),
            e => Ok(e),
        }?;
        Ok(From::from(zx::Handle::from_raw(handle)))
    }
}

const IOCTL_QMI_GET_CHANNEL: raw::c_int = make_ioctl!(
    fdio_sys::IOCTL_KIND_GET_HANDLE,
    fdio_sys::IOCTL_FAMILY_QMI,
    0
);

pub fn connect_qmi_transport(path: PathBuf) -> Result<fasync::Channel, zx::Status> {
    let file = File::open(&path)?;
    let chan = connect_qmi_transport_device(&file)?;
    Ok(fasync::Channel::from_channel(chan)?)
}

pub async fn start_qmi_modem(chan: fasync::Channel) -> Result<QmiModemProxy, Error> {
    let launcher = Launcher::new()
        .context("Failed to open launcher service")?;
    let qmi = launcher
        .launch(String::from("qmi-modem"), None)
        .context("Failed to launch qmi-modem service")?;
    let app = qmi.connect_to_service(QmiModemMarker)?;
    let success = await!(app.connect_transport(chan.into()))?;
    Ok(app)
}

pub struct Manager {
    proxies: Vec<QmiModemProxy>
}

impl Manager {
    pub fn new() -> Self {
        Manager {
            proxies: vec![],
        }
    }

    //async fn watch_new_devices<P: AsRef<Path>>(&mut self, path: P) -> Result<(), Error> {
    async fn watch_new_devices(&mut self) -> Result<(), Error> {
        let path: &Path = Path::new(QMI_TRANSPORT);
        let dir = File::open(QMI_TRANSPORT).unwrap();
        let mut watcher = Watcher::new(&dir).unwrap();
        while let Some(msg) = await!(watcher.try_next())? {
            match msg.event {
                WatchEvent::EXISTING | WatchEvent::ADD_FILE => {
                    let qmi_path = path.join(msg.filename);
                    fx_log_info!("Found QMI device at {:?}", qmi_path);
                    let channel = connect_qmi_transport(qmi_path)?;
                    let svc = await!(start_qmi_modem(channel))?;
                    self.proxies.push(svc);

                    //fx_log_info!("Connected a channel to the device");
                    //channel.write(&[0x01,
                    //              0x0F, 0x00,  // length
                    //              0x00,         // control flag
                    //              0x00,         // service type
                    //              0x00,         // client id
                    //            // SDU below
                    //              0x00, // control flag
                    //              0x00, // tx id
                    //              0x20, 0x00, // message id
                    //              0x04, 0x00, // Length
                    //              0x01, // type
                    //              0x01, 0x00, // length
                    //              0x48, // value
                    //], &mut Vec::new()); // TODO Remove
                    //let mut buffer = zx::MessageBuf::new();
                    //await!(channel.repeat_server(|_chan, buf| {
                    //    println!("{:X?}", buf.bytes());
                    //}));
                    //}
                    //self.channel = Some(channel);
                }
                _ => ()
            }
        }
        Ok(())
    }
}

fn main() -> Result<(), Error> {
    syslog::init_with_tags(&["modem-mgr"]).expect("Can't init logger");
    fx_log_info!("Starting modem-mgr...");
    let mut executor = fasync::Executor::new().context("Error creating executor")?;

    let mut manager = Manager::new();

    let device_watcher = manager.watch_new_devices();

    executor.run_singlethreaded(device_watcher)
}
