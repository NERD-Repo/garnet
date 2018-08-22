// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use clap::{App, AppSettings, Arg};
use failure::{Error, ResultExt};
use fidl_fuchsia_developer_open::OpenerMarker;
use fuchsia_app::client::connect_to_service;
use fuchsia_async as fasync;

fn main() -> Result<(), Error> {
    let _matches = App::new("fargo")
        .version("v0.2.0")
        .setting(AppSettings::GlobalVersion)
        .about("open is a command line tool for making things happen on Fuchsia.")
        .arg(
            Arg::with_name("things_to_open")
                .index(1)
                .multiple(true)
                .required(true),
        ).get_matches();

    let _core = fasync::Executor::new().context("unable to create executor")?;

    let opener = connect_to_service::<OpenerMarker>().context("Failed to connect to OpenerService")?;

    opener.open("spinning_square_view").context("failed on open call")?;

    Ok(())
}
