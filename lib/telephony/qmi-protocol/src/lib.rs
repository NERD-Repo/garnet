// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use bytes::{Bytes, Buf};
use std::fmt::Debug;
use failure::{Fail, Error};
use std::result;

pub type QmiResult<T> = result::Result<T, Error>;

pub trait Encodable {
    fn to_bytes(&self) -> (Bytes, u16);

    fn transaction_id_len(&self) -> u8;

    fn svc_id(&self) -> u8;
}

pub trait Decodable {
    fn from_bytes<T: Buf + Debug, F: Fail>(b: T) -> Result<Self, F> where Self: Sized;
}
