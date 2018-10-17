// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::env;
use std::fs;
use std::io::Read;

use failure::Error;
use crate::ast::ServiceSet;

pub mod codegen;
pub mod ast;

