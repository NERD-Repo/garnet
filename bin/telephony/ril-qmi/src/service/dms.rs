// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use crate::QmiClientPtr;
use fidl::endpoints::ServerEnd;
use fidl_fuchsia_telephony_qmi::{DeviceManagementMarker, DeviceManagementRequest};
use fuchsia_async as fasync;
use fuchsia_syslog::macros::*;
use futures::{TryFutureExt, TryStreamExt};

pub struct DataManagementService;
impl DataManagementService {
    pub fn spawn(server_end: ServerEnd<DeviceManagementMarker>, client: QmiClientPtr) {
        if let Ok(request_stream) = server_end.into_stream() {
            fasync::spawn(
                request_stream
                    .try_for_each(move |req| Self::handle_request(client.clone(), req))
                    .unwrap_or_else(|e| fx_log_err!("Error running {:?}", e)),
            );
        }
    }

    async fn handle_request(
        _client: QmiClientPtr, request: DeviceManagementRequest,
    ) -> Result<(), fidl::Error> {
        match request {
            DeviceManagementRequest::SetEventReport {
                power_state: _,
                battery_lvl_lower_limit: _,
                battery_lvl_upper_limit: _,
                pin_state: _,
                activation_state: _,
                operator_mode_state: _,
                uim_state: _,
                responder: _,
            } => Ok(()),
        }
    }
}
