// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use crate::fasync;
use failure::ResultExt;
use futures::channel::mpsc;
use futures::StreamExt;
use parking_lot::RwLock;
use std::sync::Arc;

use crate::common::sl4f::method_to_fidl;
use crate::common::sl4f::Sl4f;
use crate::common::sl4f_types::{AsyncRequest, AsyncResponse};

pub fn run_fidl_loop(
    sl4f_session: Arc<RwLock<Sl4f>>, mut receiver: mpsc::UnboundedReceiver<AsyncRequest>,
) {
    let mut executor = fasync::Executor::new()
        .context("Error creating event loop")
        .expect("Failed to create an executor!");

    // TODO another pattern for concurrent?

    let fut = async {
        while let Some(request) = await!(receiver.next()) {
            match request {
                AsyncRequest {
                    tx,
                    id,
                    method_type,
                    name,
                    params,
                } => {
                    let curr_sl4f_session = sl4f_session.clone();
                    fx_log_info!(tag: "run_fidl_loop",
                                 "Received synchronous request: {:?}, {:?}, {:?}, {:?}, {:?}",
                                 tx, id, method_type, name, params
                                );

                    let resp = await!(method_to_fidl(
                        method_type.clone(),
                        name.clone(),
                        params.clone(),
                        curr_sl4f_session.clone()));
                    let response = AsyncResponse::new(resp);

                    // Ignore any tx sending errors, other requests can still be outstanding
                    let _ = tx.send(response);
                }
            }
        }
    };

    executor.run_singlethreaded(fut);
}
