// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use crate::transport::{ClientId, SvcId};
use crate::transport::{QmiResponse, QmiTransport};
use failure::Error;
use fuchsia_syslog::macros::*;
use fuchsia_zircon as zx;
use std::sync::Arc;

pub struct QmiClient {
    inner: Arc<QmiTransport>,
}

impl QmiClient {
    pub fn new(inner: Arc<QmiTransport>) -> Self {
        QmiClient { inner: inner }
    }

    /// Send a raw QMI SDU
    ///
    /// TODO add structured data to this that doesn't take the ids as parameters as well
    /// like a header or something
    pub async fn send_raw_msg<'a>(
        &'a self, client_id: u8, svc_id: u8, buf: &'a [u8],
    ) -> Result<zx::MessageBuf, Error> {
        fx_log_info!("Sending Raw QMI message");
        let tx_id = self
            .inner
            .register_interest(SvcId(svc_id), ClientId(client_id));

        if let Some(ref transport) = self.inner.transport_channel {
            if transport.is_closed() {
                fx_log_err!("Transport channel to modem is closed");
            }
            transport.write(buf, &mut Vec::new())?;
        }

        await!(QmiResponse {
            client_id: ClientId(client_id),
            svc_id: SvcId(svc_id),
            tx_id: tx_id,
            transport: Some(self.inner.clone())
        })
    }
}
