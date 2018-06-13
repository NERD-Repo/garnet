// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

extern crate fidl;
extern crate failure;
extern crate fuchsia_app as component;
extern crate fuchsia_async as async;
extern crate futures;

use failure::{Error, ResultExt};
use futures::future::empty;

fn main() -> Result<(), Error> {
    let mut executor = async::Executor::new().context("Error creating executor")?;

    executor.run_singlethreaded(empty::<(), Error>())?;

    Ok(())
}
