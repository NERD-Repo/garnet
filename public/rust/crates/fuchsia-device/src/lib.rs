// Copyright 2016 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Type-safe bindings for Zircon kernel
//! [syscalls](https://fuchsia.googlesource.com/zircon/+/master/docs/syscalls.md).

#![deny(warnings)]
#![allow(dead_code)]
extern crate fuchsia_zircon as zx;
#[macro_use]
extern crate fdio;
#[macro_use]
extern crate failure;

pub mod pty;
