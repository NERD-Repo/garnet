// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#![feature(const_fn)]
#![feature(nonzero)]
#![feature(proc_macro, proc_macro_non_items)]
#![feature(repr_transparent)]

extern crate byteorder;
extern crate ip_macro;
extern crate zerocopy;

mod device;
mod ip;
mod queue;
mod transport;
mod wire;

fn main() {}
