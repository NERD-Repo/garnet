// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use bytes::Buf;
use std::fmt::Debug;
use fdio::{fdio_sys, ioctl_raw, make_ioctl};
use std::fs::File;
use std::os::unix::io::AsRawFd;
use std::os::raw;
use fuchsia_zircon as zx;

pub fn connect_transport_device(device: &File) -> Result<zx::Channel, zx::Status> {
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


// TEMPORARY Structures; to be codegen'd
// testing CTL structures

#[derive(Debug)]
pub struct QmuxHeader {
    pub length: u16,
    pub ctrl_flags: u8,
    pub svc_type: u8,
    pub client_id: u8,
    // general service header
    pub svc_ctrl_flags: u8,
    pub transaction_id: u16, // TODO this needs to be u16 for anything not a CTL
}

pub fn parse_qmux_header<T: Buf>(mut buf: T) -> (QmuxHeader, T) {
    assert_eq!(0x01, buf.get_u8()); // QMUX headers start with 0x01
    let length = buf.get_u16_le();
    let ctrl_flags = buf.get_u8();
    let svc_type = buf.get_u8();
    let client_id = buf.get_u8();
    let svc_ctrl_flags = buf.get_u8();
    let transaction_id;
    if (svc_type == 0x00) {
        // ctl service is one byte
        transaction_id = buf.get_u8() as u16;
    } else {
        transaction_id = buf.get_u16_le();
    }
    (QmuxHeader {
        length,
        ctrl_flags,
        svc_type,
        client_id,
        svc_ctrl_flags,
        transaction_id,
    }, buf)
}

#[derive(Debug)]
pub struct QmiSetInstanceIdResp {
    qmi_id: u16
}

#[derive(Debug)]
pub struct QmiGetClientIdResp {
    svc_id: u8,
    client_id: u8
}

pub fn parse_set_instance_id_resp<T: Buf + Debug>(mut buf: T) -> (QmiSetInstanceIdResp, T) {
    assert_eq!(0x20, buf.get_u16_le());
    let _ = buf.get_u16_le();
    assert_eq!(0x02, buf.get_u8()); //result type
    let _ = buf.get_u16_le();
    assert_eq!(0x00, buf.get_u16_le()); // no error
    assert_eq!(0x00, buf.get_u16_le()); // no error
    assert_eq!(0x01, buf.get_u8()); // instance type
    let _ = buf.get_u16_le();
    let qmi_id = buf.get_u16_le();
    (QmiSetInstanceIdResp {
        qmi_id
    }, buf)
}

pub fn parse_get_client_id<T: Buf + Debug>(mut buf: T) -> (QmiGetClientIdResp, T) {
    assert_eq!(0x22, buf.get_u16_le());
    let length = buf.get_u16_le();
    assert_eq!(0x02, buf.get_u8()); //result type
    let length2 = buf.get_u16_le();
    assert_eq!(0x00, buf.get_u16_le()); // no error
    assert_eq!(0x00, buf.get_u16_le()); // no error
    assert_eq!(0x01, buf.get_u8()); // instance type
    let length3 = buf.get_u16_le();
    let svc_id = buf.get_u8();
    let client_id = buf.get_u8();
    (QmiGetClientIdResp {
        svc_id,
        client_id,
    }, buf)
}
