// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//#![deny(warnings)]

use std::env;
use std::fs;
use std::io::Read;

use failure::Error;
use crate::ast::ServiceSet;

mod codegen;
mod ast;

fn usage(exe: &str) {
    println!("usage: -i {} <qmi json defs> -o <protocol.rs>", exe);
    println!("");
    println!("Generates bindings for QMI");
    ::std::process::exit(1);
}

fn main() -> Result<(), Error> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        usage(&args[0]);
    }

    if !args.contains(&String::from("-i")) && !args.contains(&String::from("-o")) {
        usage(&args[0]);
    }

    println!("{:?}", args);

    let mut svc_set = ServiceSet::new();

    let file = fs::File::create(&args[4])?;
    let mut c = codegen::Codegen::new(file);

    // for each input
    let mut svc_file = fs::File::open(&args[2])?;
    svc_set.parse_service_file(svc_file)?;

    c.codegen(svc_set)
}
