// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! System service for managing cellular modems

#![feature(
    async_await,
    await_macro,
    futures_api,
    arbitrary_self_types,
    pin
)]
//#![deny(warnings)]
//#![deny(missing_docs)]
//
//

use failure::{Error, ResultExt};
use fdio::{fdio_sys, ioctl_raw, make_ioctl};
use fidl::endpoints::{RequestStream, ServiceMarker};
use fidl_fuchsia_telephony_manager::{ManagerMarker, ManagerRequest, ManagerRequestStream};
use fidl_fuchsia_telephony_qmi::{QmiClientMarker, QmiClientProxy};
use fidl_fuchsia_telephony_qmi::{QmiModemMarker, QmiModemProxy};
use fuchsia_app::{client::Launcher, server::ServicesServer};
use fuchsia_async as fasync;
use fuchsia_syslog::{self as syslog, macros::*};
use fuchsia_vfs_watcher::{WatchEvent, Watcher};
use fuchsia_zircon as zx;
use futures::{future, Future, Stream, TryFutureExt, TryStreamExt};
use parking_lot::RwLock;
use std::fs::File;
use std::io;
use std::os::raw;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use qmi::connect_transport_device;

const QMI_TRANSPORT: &str = "/dev/class/qmi-transport";

//pub fn connect_qmi_transport_device(device: &File) -> Result<zx::Channel, zx::Status> {
//    let mut handle: zx::sys::zx_handle_t = zx::sys::ZX_HANDLE_INVALID;
//
//    // This call is safe because the callee does not retain any data from the call, and the return
//    // value ensures that the handle is a valid handle to a zx::channel.
//    unsafe {
//        match ioctl_raw(
//            device.as_raw_fd(),
//            IOCTL_QMI_GET_CHANNEL,
//            ::std::ptr::null(),
//            0,
//            &mut handle as *mut _ as *mut raw::c_void,
//            ::std::mem::size_of::<zx::sys::zx_handle_t>(),
//        ) as i32
//        {
//            e if e < 0 => Err(zx::Status::from_raw(e)),
//            e => Ok(e),
//        }?;
//        Ok(From::from(zx::Handle::from_raw(handle)))
//    }
//}
//
//const IOCTL_QMI_GET_CHANNEL: raw::c_int = make_ioctl!(
//    fdio_sys::IOCTL_KIND_GET_HANDLE,
//    fdio_sys::IOCTL_FAMILY_QMI,
//    0
//);

pub fn connect_qmi_transport(path: PathBuf) -> Result<fasync::Channel, zx::Status> {
    let file = File::open(&path)?;
    let chan = connect_transport_device(&file)?;
    Ok(fasync::Channel::from_channel(chan)?)
}

pub async fn start_qmi_modem(chan: fasync::Channel) -> Result<QmiModemProxy, Error> {
    let launcher = Launcher::new().context("Failed to open launcher service")?;
    let qmi = launcher
        .launch(String::from("qmi-modem"), None)
        .context("Failed to launch qmi-modem service")?;
    let app = qmi.connect_to_service(QmiModemMarker)?;
    let success = await!(app.connect_transport(chan.into()))?;
    fx_log_info!("connected transport: {}", success);

    //let (client, remote) = zx::Channel::create().unwrap();
    //let client = fasync::Channel::from_channel(client).unwrap();
    //let server = fidl::endpoints::ServerEnd::<QmiClientMarker>::new(remote);

    //let client_resp = await!(app.connect_client(server));
    //eprintln!("Client connection: {:?}", client_resp);
    Ok(app)
}

pub fn start_service(
    mgr: Arc<Manager>, channel: fasync::Channel,
) -> impl Future<Output = Result<(), Error>> {
    let stream = ManagerRequestStream::from_channel(channel);
    stream
        .try_for_each(move |evt| {
            match evt {
                ManagerRequest::IsAvailable { responder } => {
                    responder.send(!mgr.proxies.read().is_empty())
                }
                ManagerRequest::GetModemHandle { ril, responder } => responder.send(true),
            };
            future::ready(Ok(()))
        }).map_err(|e| e.into())
}

pub struct Manager {
    proxies: RwLock<Vec<QmiModemProxy>>,
}

impl Manager {
    pub fn new() -> Self {
        Manager {
            proxies: RwLock::new(vec![]),
        }
    }

    //async fn watch_new_devices<P: AsRef<Path>>(&mut self, path: P) -> Result<(), Error> {
    async fn watch_new_devices(&self) -> Result<(), Error> {
        let path: &Path = Path::new(QMI_TRANSPORT);
        let dir = File::open(QMI_TRANSPORT).unwrap();
        let mut watcher = Watcher::new(&dir).unwrap();
        while let Some(msg) = await!(watcher.try_next())? {
            match msg.event {
                WatchEvent::EXISTING | WatchEvent::ADD_FILE => {
                    let qmi_path = path.join(msg.filename);
                    fx_log_info!("Found QMI device at {:?}", qmi_path);
                    let channel = connect_qmi_transport(qmi_path)?;
                    fx_log_info!("Client connection: {:?}", channel);
                    let svc = await!(start_qmi_modem(channel))?;
                    fx_log_info!("Started modem: {:?}", svc);

                    self.proxies.write().push(svc);
                }
                _ => (),
            }
        }
        Ok(())
    }
}

fn main() -> Result<(), Error> {
    syslog::init_with_tags(&["modem-mgr"]).expect("Can't init logger");
    fx_log_info!("Starting modem-mgr...");
    let mut executor = fasync::Executor::new().context("Error creating executor")?;

    // TODO investigate pin_mut!
    let mut manager = Arc::new(Manager::new());
    let mgr = manager.clone();
    let device_watcher = manager.watch_new_devices();

    let server = ServicesServer::new()
        .add_service((ManagerMarker::NAME, move |chan: fasync::Channel| {
            fx_log_info!("Spawning Management Interface");
            fasync::spawn(
                start_service(mgr.clone(), chan)
                    .unwrap_or_else(|e| eprintln!("Failed to spawn {:?}", e)),
            )
        })).start()?;

    executor
        .run_singlethreaded(device_watcher.try_join(server))
        .map(|_| ())
}
