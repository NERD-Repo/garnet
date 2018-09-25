// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#![feature(async_await, await_macro, futures_api)]

use futures::prelude::*;
use fuchsia_zircon::prelude::*;
use fuchsia_async as fasync;
use fuchsia_zircon as zx;

async fn create_future() -> () {
    println!("This will print second.");
    let res = await!(wait_1_second());
    println!("{:?}", res);
    ()
}

fn main() {
    println!("Hello, Fuchsia 2!");

    let mut executor = fuchsia_async::Executor::new()
        .expect("Creating fuchsia_async executor for tennis service failed");

    let fut = create_future();
    println!("This will print first!");
    executor.run_singlethreaded(fut);
    println!("This will print last!");
}

async fn wait_1_second() -> () {
    // TODO use fasync::Timer::new to wait before returning;
    let time_step: i64 = 1000;
    await!(fasync::Timer::new(time_step.millis().after_now()));
}
