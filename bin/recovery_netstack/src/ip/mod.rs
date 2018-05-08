// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

mod address;
mod forwarding;

pub use self::address::*;

/// An IP protocol or next header number.
///
/// For IPv4, this is the protocol number. For IPv6, this is the next header
/// number.
#[repr(u8)]
pub enum IpProto {
    Tcp = 6,
    Udp = 17,
}

pub struct Ipv4Option {
    pub copied: bool,
    // TODO: include "Option Class"?
    pub inner: Ipv4OptionInner,
}

pub enum Ipv4OptionInner {
    // According to https://myweb.ntut.edu.tw/~kwke/DC2006/ipo.pdf, maximum IPv4
    // option length is 40 bytes, leaving 38 bytes for data.
    Unrecognized { kind: u8, len: u8, data: [u8; 38] },
}
