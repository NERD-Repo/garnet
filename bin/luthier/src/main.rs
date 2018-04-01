// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#![deny(warnings)]

//#[macro_use]
extern crate failure;
// #[macro_use]
extern crate fdio;
extern crate fuchsia_async as async;
extern crate futures;
extern crate fidl_luthier;

#[macro_use]
extern crate structopt;

use failure::Error;
use std::path::PathBuf;
use structopt::StructOpt;

/// A basic example
#[derive(StructOpt, Debug)]
#[structopt(name = "basic")]
struct Opt {
    #[structopt(short = "v", long = "verbose", parse(from_occurrences))]
    verbose: u8,

    #[structopt(short = "d", long = "directory", parse(from_os_str))]
    startup_directory: Option<PathBuf>,

    #[structopt(name = "FILE", parse(from_os_str))]
    fidl_files: Vec<PathBuf>,
}

fn startup_luthier() -> Result<(), Error> {


    Ok(())
}

fn main() {
    let opt = Opt::from_args();
    if opt.fidl_files.is_empty() {
        startup_luthier().unwrap(); //TODO error
    }
    println!("{:?}", opt);
}
