// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//#![deny(warnings)]

use std::env;
use std::fs;

mod codegen;

fn usage(exe: &str) {
    println!("usage: -i {} <qmi json defs> -o <protocol.rs>", exe);
    println!("");
    println!("Generates bindings for QMI");
    ::std::process::exit(1);
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        usage(&args[0]);
    }

    if !args.contains(&String::from("-i")) && !args.contains(&String::from("-o")) {
        usage(&args[0]);
    }

    println!("{:?}", args);
    if let Ok(file) = fs::File::create(&args[4]) {
        let mut c = codegen::Codegen::new(file);
//        c.codegen();
    }
}
