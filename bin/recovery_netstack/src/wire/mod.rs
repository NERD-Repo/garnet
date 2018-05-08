// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Serialization and deserialization of wire formats.
//!
//! This module provides efficient serialization and deserialization of the
//! various wire formats used by this program. Where possible, it uses lifetimes
//! and immutability to allow for safe zero-copy parsing.

// We use repr(packed) in this module to create structs whose layout matches the
// layout of network packets on the wire. This ensures that the compiler will
// stop us from using repr(packed) in an unsound manner without using unsafe
// code.
#![deny(safe_packed_borrows)]

#[macro_use]
mod macros;
mod ethernet;
mod ipv4;
mod tcp;
mod udp;
mod util;

pub use self::ipv4::*;
pub use self::tcp::*;
pub use self::udp::*;
