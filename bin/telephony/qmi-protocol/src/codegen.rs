// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::io;
//use crate::ast;
//use crate::parser::ArgKind;

pub type Result = io::Result<()>;

pub struct Codegen<W: io::Write> {
    w: W,
}

impl<W: io::Write> Codegen<W> {
    pub fn new(w: W) -> Codegen<W> {
        Codegen { w }
    }

    pub fn codegen(&mut self) -> Result {
        writeln!(self.w, "CODEGEN'd. PLACEHOLDER. EXPERIMENT PHASE")
    }
}
