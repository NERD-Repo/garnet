// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use bytes::{BufMut, BytesMut};
use crate::transport::{ClientId, SvcId};
use crate::transport::{QmiResponse, QmiTransport};
use fuchsia_syslog::macros::*;
use qmi_protocol::QmiResult;
use crate::errors::QmuxError;
use qmi_protocol::{Decodable, Encodable};
use std::fmt::Debug;
use std::sync::Arc;

pub struct QmiClient {
    inner: Arc<QmiTransport>,
    client_id: ClientId,
}

impl QmiClient {
    pub fn new(inner: Arc<QmiTransport>) -> Self {
        QmiClient {
            inner: inner,
            client_id: ClientId::default(),
        }
    }

    /// Send a QMI message
    pub async fn send_msg<'a, E: Encodable + 'a, D: Decodable + Debug>(
        &'a self, msg: E,
    ) -> Result<QmiResult<D>, QmuxError> {
        fx_log_info!("Sending a structured QMI message");
        let tx_id = self
            .inner
            .register_interest(SvcId(msg.svc_id()), self.client_id);

        let mut msg_buf = BytesMut::new();
        let (payload_bytes, payload_len) = msg.to_bytes();
        // QMI header
        msg_buf.put_u8(0x01); // magic QMI number
                              // 2 bytes total length
        msg_buf.put_u16_le(
            payload_len
                           + 3 /* flags */
                           + 2 /* length byte length */
                           // additional length is bytes not captured in the payload length
                           // They cannot be calculated there because multi-payload SDUs may
                           // exist
                           + 1 /* sdu control flag */
                           + msg.transaction_id_len() as u16,
        );

        // 1 byte control flag
        msg_buf.put_u8(0x00);
        // 1 byte svc flag
        msg_buf.put_u8(msg.svc_id());
        // 1 byte client id
        msg_buf.put_u8(self.client_id.0);

        // SDU
        // 1 byte control flag
        msg_buf.put_u8(0x00);
        // 1 or 2 byte transaction ID
        match msg.transaction_id_len() {
            1 => msg_buf.put_u8(tx_id.0 as u8), // we know it's one byte
            2 => msg_buf.put_u16_le(tx_id.0),
            _ => panic!(
                "Unknown transaction ID length. Please add client support or fix the message \
                 definitions"
            ),
        }
        // add the payload to the buffer
        msg_buf.extend(payload_bytes);

        let bytes = msg_buf.freeze();
        eprintln!("byte payload {:X?}", bytes.as_ref());

        if let Some(ref transport) = self.inner.transport_channel {
            if transport.is_closed() {
                fx_log_err!("Transport channel to modem is closed");
            }
            transport.write(bytes.as_ref(), &mut Vec::new()).map_err(QmuxError::ClientWrite)?
        }

        let resp = await!(QmiResponse {
            client_id: self.client_id,
            svc_id: SvcId(msg.svc_id()),
            tx_id: tx_id,
            transport: Some(self.inner.clone())
        })?;

        eprintln!("response {:?}", resp);

        let buf = std::io::Cursor::new(resp.bytes());
        let decoded = D::from_bytes(buf);
        eprintln!("decoded: {:?}", decoded);
        Ok(decoded)
    }
}
