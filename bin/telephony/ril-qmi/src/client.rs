// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::pin::{Unpin, Pin};
use bytes::{BufMut, BytesMut};
use crate::transport::{ClientId, SvcId};
use crate::transport::{QmiResponse, QmiTransport};
use std::collections::HashMap;
use fuchsia_syslog::macros::*;
use qmi_protocol::QmiResult;
use crate::errors::QmuxError;
use qmi_protocol::{Decodable, Encodable};
use parking_lot::RwLock;
use std::ops::Deref;
use std::fmt::Debug;
use std::sync::Arc;

#[derive(Debug)]
pub struct ClientSvcMap(RwLock<HashMap<SvcId, ClientId>>);

impl Default for ClientSvcMap {
    fn default() -> Self {
        let mut m = HashMap::new();
        // this allows the client to request svc -> client
        m.insert(SvcId(0), ClientId(0));
        ClientSvcMap(RwLock::new(m))
    }
}
impl Deref for ClientSvcMap {
    type Target = RwLock<HashMap<SvcId, ClientId>>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug)]
pub struct QmiClient {
    inner: Arc<QmiTransport>,
    clients: ClientSvcMap,
}

impl Unpin for QmiClient {}

impl QmiClient {
    pub fn new(inner: Arc<QmiTransport>) -> Self {
        QmiClient {
            inner: inner,
            clients: ClientSvcMap::default(),
        }
    }

    /// Send a QMI message and allocate the client IDs for the service
    /// if they have not yet been
    pub async fn send_msg<'a, E: Encodable + 'a, D: Decodable + Debug>(
        &'a self, msg: E,
    ) -> Result<QmiResult<D>, QmuxError> {
        let svc_id = SvcId(msg.svc_id());
        let mut request_id = false;
        {
            let mut map = self.clients.read();
            // allocate a client id for this service
            if map.get(&svc_id).is_none() {
                request_id = true;
            }
        }
        if request_id {
            use qmi_protocol::CTL::{GetClientIdReq, GetClientIdResp};
            fx_log_info!("allocating a client ID for service: {}", svc_id.0);
            let resp: QmiResult<GetClientIdResp> = await!(self.send_msg_actual(GetClientIdReq::new(svc_id.0)))?;
            let client_id_resp = resp.unwrap(); // TODO from trait for QmiError to QmuxError
            let mut map = self.clients.write();
            assert_eq!(client_id_resp.svc_type, svc_id.0);
            map.insert(svc_id, ClientId(client_id_resp.client_id));
        }
        Ok(await!(self.send_msg_actual(msg))?)
    }

    fn get_client_id(&self, svc_id: SvcId) -> ClientId {
        let clients = self.clients.read();
        if let Some(id) = clients.get(&svc_id) {
            return *id;
        }
        panic!("Precondition of calling get_client_id is to have verified an ID is allocated");
    }

    /// Send a QMI message without checking if a client ID has been allocated for the service
    async fn send_msg_actual<'a, E: Encodable + 'a, D: Decodable + Debug>(
        &'a self, msg: E,
    ) -> Result<QmiResult<D>, QmuxError> {
        fx_log_info!("Sending a structured QMI message");

        let svc_id = SvcId(msg.svc_id());
        let client_id = self.get_client_id(svc_id);

        let tx_id = self
            .inner
            .register_interest(svc_id, client_id);

        let mut msg_buf = BytesMut::new();
        let (payload_bytes, payload_len) = msg.to_bytes();
        // QMI header
        msg_buf.put_u8(0x01); // magic QMI number
                              // 2 bytes total length
        msg_buf.put_u16_le(payload_len
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
        msg_buf.put_u8(svc_id.0);
        // 1 byte client id
        msg_buf.put_u8(client_id.0);

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
        // eprintln!("byte payload {:X?}", bytes.as_ref());

        if let Some(ref transport) = self.inner.transport_channel {
            if transport.is_closed() {
                fx_log_err!("Transport channel to modem is closed");
            }
            transport.write(bytes.as_ref(), &mut Vec::new()).map_err(QmuxError::ClientWrite)?
        }

        let resp = await!(QmiResponse {
            client_id: client_id,
            svc_id: svc_id,
            tx_id: tx_id,
            transport: Some(self.inner.clone())
        })?;

        // eprintln!("response {:?}", resp);

        let buf = std::io::Cursor::new(resp.bytes());
        let decoded = D::from_bytes(buf);
        eprintln!("decoded: {:?}", decoded);
        Ok(decoded)
    }
}
