// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#![deny(warnings)]
#![feature(
    arbitrary_self_types,
    async_await,
    await_macro,
    futures_api,
    pin
)]

#[macro_use]
extern crate failure;

use fuchsia_async as fasync;
use fuchsia_zircon as zx;
use futures::ready;
use futures::stream::Stream;
use futures::task::{LocalWaker, Poll, Waker};
use parking_lot::Mutex;
use slab::Slab;
use std::collections::VecDeque;
use std::io::{Cursor, Write};
use std::mem;
use std::pin::{Pin, Unpin};
use std::sync::Arc;

#[cfg(test)]
mod tests;

mod types;

pub use crate::types::*;

/// An AVDTP peer which can send commands to another peer and receive requests
/// and send responses.
///
/// Requests from the distant peer are delivered through the request stream available
/// through take_request_stream().  Only one RequestStream can be active at a time.
/// Only valid requests are sent to the request stream - invalid formats are
/// automatically rejected.
///
/// Responses are sent using responders that are included in the request stream
/// from the connected peer.
///
/// Media transport is not handled by this library.
#[derive(Debug, Clone)]
pub struct Peer {
    inner: Arc<PeerInner>,
}

impl Peer {
    /// Create a new peer from a signaling channel socket.
    pub fn new(signaling: zx::Socket) -> Result<Peer, zx::Status> {
        Ok(Peer {
            inner: Arc::new(PeerInner {
                signaling: fasync::Socket::from_socket(signaling)?,
                response_waiters: Mutex::new(Slab::<ResponseWaiter>::new()),
                waiting_requests: Mutex::<RequestQueue>::default(),
            }),
        })
    }

    /// Take the event listener for this peer. Panics if the stream is already
    /// held.
    pub fn take_request_stream(&self) -> RequestStream {
        {
            let mut lock = self.inner.waiting_requests.lock();
            if let RequestListener::None = lock.listener {
                lock.listener = RequestListener::New;
            } else {
                panic!("Request stream has already been taken");
            }
        }

        RequestStream {
            inner: self.inner.clone(),
        }
    }

    pub async fn discover(&self) -> Result<DiscoverResponse, Error> {
        await!(self.send_command(SignalIdentifier::Discover, &[]))
    }

    /// Send a signal on the socket and receive a future that will complete
    /// when we get the expected reponse.
    async fn send_command<'a, D: Decodable>(
        &'a self, signal_id: SignalIdentifier, payload: &'a [u8],
    ) -> Result<D, Error> {
        let id = self.inner.add_waiter()?;
        let header = SignalingHeader {
            txid: id,
            signal_id: signal_id,
            message_type: SignalingMessageType::Command,
            packet_type: SignalingPacketType::Single,
            num_packets: 1,
        };

        {
            let mut buf = vec![0; header.size()];

            header.encode(buf.as_mut_slice())?;
            buf.extend_from_slice(payload);

            self.inner.send_signal(buf.as_slice())?;
        }

        let response_buf = await!(CommandResponse {
            id: header.txid,
            inner: Some(self.inner.clone()),
        })?;

        decode_signaling_response(header.signal_id, response_buf)
    }
}

/// A request from the connected peer.
/// Each variant of this includes a responder which implements two functions:
///  - send(...) will send a response with the information provided.
///  - reject(ErrorCode) will send an reject response with the given error code.
#[derive(Debug)]
pub enum Request {
    Discover {
        responder: DiscoverResponder,
    },
    GetCapabilities {
        stream_id: StreamEndpointId,
        responder: GetCapabilitiesResponder,
    },
    GetAllCapabilities {
        stream_id: StreamEndpointId,
        responder: GetCapabilitiesResponder,
    },
    SetConfiguration {
        local_stream_id: StreamEndpointId,
        remote_stream_id: StreamEndpointId,
        capabilities: Vec<ServiceCapability>,
        responder: SimpleResponder,
    },
    Open {
        stream_id: StreamEndpointId,
        responder: SimpleResponder,
    },
    Close {
        stream_id: StreamEndpointId,
        responder: SimpleResponder,
    },
    Start {
        stream_ids: Vec<StreamEndpointId>,
        responder: SimpleResponder,
    },
    Suspend {
        stream_ids: Vec<StreamEndpointId>,
        responder: SimpleResponder,
    },
    // TODO(jamuraa): add the rest of the requests
}

impl Request {
    fn parse(
        peer: Arc<PeerInner>, id: TxId, signal_id: SignalIdentifier, body: &[u8],
    ) -> Result<Request, Error> {
        match signal_id {
            SignalIdentifier::Discover => {
                if body.len() > 0 {
                    return Err(Error::BadLength);
                }
                Ok(Request::Discover {
                    responder: DiscoverResponder { peer: peer, id: id },
                })
            }
            SignalIdentifier::GetCapabilities => {
                if body.len() != 1 {
                    return Err(Error::BadLength);
                }
                Ok(Request::GetCapabilities {
                    stream_id: StreamEndpointId(body[0] >> 2),
                    responder: GetCapabilitiesResponder {
                        signal: signal_id,
                        peer: peer,
                        id: id,
                    },
                })
            }
            SignalIdentifier::GetAllCapabilities => {
                if body.len() != 1 {
                    return Err(Error::BadLength);
                }
                Ok(Request::GetAllCapabilities {
                    stream_id: StreamEndpointId(body[0] >> 2),
                    responder: GetCapabilitiesResponder {
                        signal: signal_id,
                        peer: peer,
                        id: id,
                    },
                })
            }
            SignalIdentifier::SetConfiguration => {
                if body.len() < 4 {
                    return Err(Error::BadLength);
                }
                let mut caps = Vec::<ServiceCapability>::new();
                let mut loc = 2;
                while loc < body.len() {
                    let cap = ServiceCapability::decode(&body[loc..])?;
                    loc += cap.encoded_size();
                    caps.push(cap);
                }
                Ok(Request::SetConfiguration {
                    local_stream_id: StreamEndpointId(body[0] >> 2),
                    remote_stream_id: StreamEndpointId(body[1] >> 2),
                    capabilities: caps,
                    responder: SimpleResponder {
                        signal: signal_id,
                        peer: peer,
                        id: id,
                    },
                })
            }
            SignalIdentifier::Open => {
                if body.len() > 1 {
                    return Err(Error::BadLength);
                }
                Ok(Request::Open {
                    stream_id: StreamEndpointId(body[0] >> 2),
                    responder: SimpleResponder {
                        signal: signal_id,
                        peer: peer,
                        id: id,
                    },
                })
            }
            SignalIdentifier::Close => {
                if body.len() > 1 {
                    return Err(Error::BadLength);
                }
                Ok(Request::Close {
                    stream_id: StreamEndpointId(body[0] >> 2),
                    responder: SimpleResponder {
                        signal: signal_id,
                        peer: peer,
                        id: id,
                    },
                })
            }
            SignalIdentifier::Start => {
                if body.len() > 1 {
                    return Err(Error::BadLength);
                }
                let mut streams = Vec::<StreamEndpointId>::new();
                let mut loc = 0;
                while loc < body.len() {
                    streams.push(StreamEndpointId(body[loc] >> 2));
                    loc += 1;
                }
                Ok(Request::Start {
                    stream_ids: streams,
                    responder: SimpleResponder {
                        signal: signal_id,
                        peer: peer,
                        id: id,
                    },
                })
            }
            SignalIdentifier::Suspend => {
                if body.len() > 1 {
                    return Err(Error::BadLength);
                }
                let mut streams = Vec::<StreamEndpointId>::new();
                let mut loc = 0;
                while loc < body.len() {
                    streams.push(StreamEndpointId(body[loc] >> 2));
                    loc += 1;
                }
                Ok(Request::Suspend {
                    stream_ids: streams,
                    responder: SimpleResponder {
                        signal: signal_id,
                        peer: peer,
                        id: id,
                    },
                })
            }
            _ => Err(Error::UnimplementedMessage),
        }
    }
}

/// A decodable type can be created from a byte buffer.
/// The type returned is separate (copied) from the buffer once decoded.
trait Decodable: Sized {
    /// Decodes into a new object, or returns an error.
    fn decode(buf: &[u8]) -> Result<Self, Error>;
}

/// A stream of requests from the remote peer.
#[derive(Debug)]
pub struct RequestStream {
    inner: Arc<PeerInner>,
}

impl Unpin for RequestStream {}

impl Stream for RequestStream {
    type Item = Result<Request, Error>;

    fn poll_next(self: Pin<&mut Self>, lw: &LocalWaker) -> Poll<Option<Self::Item>> {
        Poll::Ready(match ready!(self.inner.poll_recv_request(lw)) {
            Ok(UnparsedRequest(
                SignalingHeader {
                    txid, signal_id, ..
                },
                body,
            )) => match Request::parse(self.inner.clone(), txid, signal_id, &body) {
                Err(Error::BadLength) => {
                    self.inner
                        .send_reject(txid, signal_id, ErrorCode::BadLength)?;
                    return Poll::Pending;
                }
                Err(Error::UnimplementedMessage) => {
                    self.inner
                        .send_reject(txid, signal_id, ErrorCode::NotSupportedCommand)?;
                    return Poll::Pending;
                }
                x => Some(x),
            },
            Err(Error::PeerDisconnected) => None,
            Err(e) => Some(Err(e)),
        })
    }
}

impl Drop for RequestStream {
    fn drop(&mut self) {
        self.inner.waiting_requests.lock().listener = RequestListener::None;
        self.inner.wake_any();
    }
}

/// A Stream Endpoint Identifier, aka SEID, INT SEID, ACP SEID
/// Valid values are 0x01 - 0x3E
#[derive(Debug, PartialEq)]
pub struct StreamEndpointId(u8);

impl TryFrom<u8> for StreamEndpointId {
    type Error = Error;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        if value == 0 || value > 0x3E {
            Err(Error::OutOfRange)
        } else {
            Ok(StreamEndpointId(value))
        }
    }
}

/// All information related to a stream. Part of the Discovery Response.
#[derive(Debug, PartialEq)]
pub struct StreamInformation {
    id: StreamEndpointId,
    in_use: bool,
    media_type: MediaType,
    endpoint_type: EndpointType,
}

impl StreamInformation {
    pub fn new(
        id: u8, in_use: bool, media_type: MediaType, endpoint_type: EndpointType,
    ) -> Result<StreamInformation, Error> {
        Ok(StreamInformation {
            id: StreamEndpointId::try_from(id)?,
            in_use: in_use,
            media_type: media_type,
            endpoint_type: endpoint_type,
        })
    }

    fn size(&self) -> usize {
        2
    }

    fn encode(&self, into: &mut [u8]) -> Result<(), Error> {
        if into.len() < self.size() {
            return Err(Error::Encoding);
        }
        into[0] = self.id.0 << 2 | if self.in_use { 0x02 } else { 0x00 };
        into[1] = u8::from(&self.media_type) << 4 | u8::from(&self.endpoint_type) << 3;
        Ok(())
    }
}

impl Decodable for StreamInformation {
    fn decode(from: &[u8]) -> Result<Self, Error> {
        if from.len() < 2 {
            return Err(Error::InvalidMessage);
        }
        let id = StreamEndpointId(from[0] >> 2);
        let in_use: bool = from[0] & 0x02 != 0;
        let media_type = MediaType::try_from(from[1] >> 4)?;
        let endpoint_type = EndpointType::try_from((from[1] >> 3) & 0x1)?;
        Ok(StreamInformation {
            id: id,
            in_use: in_use,
            media_type: media_type,
            endpoint_type: endpoint_type,
        })
    }
}

/// The type of the codec in the MediaCodec Service Capability
/// Valid values are defined in the Bluetooth Assigned Numbers and are
/// interpreted differently for different Media Types, so we do not interpret
/// them here.
/// See https://www.bluetooth.com/specifications/assigned-numbers/audio-video
#[derive(Debug)]
pub struct MediaCodecType(u8);

impl MediaCodecType {
    pub fn new(num: u8) -> MediaCodecType {
        MediaCodecType(num)
    }
}

/// The type of content protection used in the Content Protection Service Capability.
/// Defined in the Bluetooth Assigned Numbers
/// https://www.bluetooth.com/specifications/assigned-numbers/audio-video
#[derive(Debug)]
pub enum ContentProtectionType {
    DigitalTransmissionContentProtection, // DTCP, 0x0001
    SerialCopyManagementSystem,           // SCMS-T, 0x0002
}

impl ContentProtectionType {
    fn to_le_bytes(&self) -> [u8; 2] {
        match self {
            ContentProtectionType::DigitalTransmissionContentProtection => [0x01, 0x00],
            ContentProtectionType::SerialCopyManagementSystem => [0x02, 0x00],
        }
    }
}

impl TryFrom<u16> for ContentProtectionType {
    type Error = Error;

    fn try_from(val: u16) -> Result<Self, Self::Error> {
        match val {
            1 => Ok(ContentProtectionType::DigitalTransmissionContentProtection),
            2 => Ok(ContentProtectionType::SerialCopyManagementSystem),
            _ => Err(Error::OutOfRange),
        }
    }
}

/// Service Capabilities indicate possible services that can be provided by
/// each stream endpoint.  See AVDTP Spec section 8.21.
#[derive(Debug)]
pub enum ServiceCapability {
    /// Indicates that the end point can provide at least basic media transport
    /// service as defined by RFC 3550 and outlined in section 7.2.
    /// Defined in section 8.21.2
    MediaTransport,
    /// Indicates that the end point can provide reporting service as outlined in section 7.3
    /// Defined in section 8.21.3
    Reporting,
    /// Indicates the end point can provide recovery service as outlined in section 7.4
    /// Defined in section 8.21.4
    Recovery {
        recovery_type: u8,
        max_recovery_window_size: u8,
        max_number_media_packets: u8,
    },
    /// Indicates the codec which is supported by this end point. |codec_extra| is defined within
    /// the relevant profiles (A2DP for Audio, etc).
    /// Defined in section 8.21.5
    MediaCodec {
        media_type: MediaType,
        codec_type: MediaCodecType,
        codec_extra: Vec<u8>, // TODO: Media codec specific information elements
    },
    /// Present when the device has content protection capability.
    /// |extra| is defined elsewhere.
    /// Defined in section 8.21.6
    ContentProtection {
        protection_type: ContentProtectionType,
        extra: Vec<u8>, // Protection speciifc parameters
    },
    /// Indicates that delay reporting is offered by this end point.
    /// Defined in section 8.21.9
    DelayReporting,
}

impl ServiceCapability {
    fn to_category_byte(&self) -> u8 {
        match self {
            ServiceCapability::MediaTransport => 1,
            ServiceCapability::Reporting => 2,
            ServiceCapability::Recovery { .. } => 3,
            ServiceCapability::ContentProtection { .. } => 4,
            ServiceCapability::MediaCodec { .. } => 7,
            ServiceCapability::DelayReporting => 8,
        }
    }

    fn length_of_service_capabilities(&self) -> u8 {
        match self {
            ServiceCapability::MediaTransport => 0,
            ServiceCapability::Reporting => 0,
            ServiceCapability::Recovery { .. } => 3,
            ServiceCapability::MediaCodec { codec_extra, .. } => 2 + codec_extra.len() as u8,
            ServiceCapability::ContentProtection { extra, .. } => 2 + extra.len() as u8,
            ServiceCapability::DelayReporting => 0,
        }
    }

    fn encoded_size(&self) -> usize {
        2 + self.length_of_service_capabilities() as usize
    }

    fn encode(&self, into: &mut [u8]) -> Result<(), Error> {
        if into.len() < self.encoded_size() {
            return Err(Error::Encoding);
        }
        let mut cursor = Cursor::new(into);
        cursor
            .write(&[
                self.to_category_byte(),
                self.length_of_service_capabilities(),
            ])
            .map_err(|_| Error::Encoding)?;
        match self {
            ServiceCapability::Recovery {
                recovery_type: t,
                max_recovery_window_size: max_size,
                max_number_media_packets: max_packets,
            } => {
                cursor
                    .write(&[*t, *max_size, *max_packets])
                    .map_err(|_| Error::Encoding)?;
            }
            ServiceCapability::MediaCodec {
                media_type,
                codec_type,
                codec_extra,
            } => {
                cursor
                    .write(&[u8::from(media_type) << 4, codec_type.0])
                    .map_err(|_| Error::Encoding)?;
                cursor
                    .write(codec_extra.as_slice())
                    .map_err(|_| Error::Encoding)?;
            }
            ServiceCapability::ContentProtection {
                protection_type,
                extra,
            } => {
                cursor
                    .write(&protection_type.to_le_bytes())
                    .map_err(|_| Error::Encoding)?;
                cursor
                    .write(extra.as_slice())
                    .map_err(|_| Error::Encoding)?;
            }
            _ => {}
        }
        Ok(())
    }
}

impl Decodable for ServiceCapability {
    fn decode(from: &[u8]) -> Result<ServiceCapability, Error> {
        if from.len() < 2 {
            return Err(Error::Encoding);
        }
        let d = match from[0] {
            1 => ServiceCapability::MediaTransport,
            2 => ServiceCapability::Reporting,
            3 => {
                if from.len() < 5 {
                    return Err(Error::Encoding);
                }
                ServiceCapability::Recovery {
                    recovery_type: from[2],
                    max_recovery_window_size: from[3],
                    max_number_media_packets: from[4],
                }
            }
            4 => {
                let prot =
                    ContentProtectionType::try_from(((from[3] as u16) << 8) + from[2] as u16)?;
                let mut extra = vec![0; from.len() - 4];
                extra.copy_from_slice(&from[4..]);
                ServiceCapability::ContentProtection {
                    protection_type: prot,
                    extra: extra,
                }
            }
            7 => {
                let media = MediaType::try_from(from[3] >> 4)?;
                let codec_type = MediaCodecType::new(from[4]);
                let mut codec_extra = vec![0; from.len() - 4];
                codec_extra.copy_from_slice(&from[4..]);
                ServiceCapability::MediaCodec {
                    media_type: media,
                    codec_type: codec_type,
                    codec_extra: codec_extra,
                }
            }
            8 => ServiceCapability::DelayReporting,
            _ => {
                return Err(Error::Encoding);
            }
        };
        Ok(d)
    }
}

#[derive(Debug)]
pub struct DiscoverResponse {
    endpoints: Vec<StreamInformation>,
}

impl Decodable for DiscoverResponse {
    fn decode(from: &[u8]) -> Result<Self, Error> {
        let mut endpoints = Vec::<StreamInformation>::new();
        let mut idx = 0;
        while idx < from.len() {
            let endpoint = StreamInformation::decode(&from[idx..])?;
            idx += endpoint.size();
            endpoints.push(endpoint);
        }
        Ok(DiscoverResponse {
            endpoints: endpoints,
        })
    }
}

#[derive(Debug)]
pub struct SimpleResponder {
    peer: Arc<PeerInner>,
    signal: SignalIdentifier,
    id: TxId,
}

impl SimpleResponder {
    pub fn send(self) -> Result<(), Error> {
        let header = SignalingHeader {
            txid: self.id,
            signal_id: self.signal,
            message_type: SignalingMessageType::ResponseAccept,
            packet_type: SignalingPacketType::Single,
            num_packets: 1,
        };
        let mut reply = vec![0 as u8; header.size()];
        header.encode(reply.as_mut_slice())?;
        self.peer.send_signal(reply.as_slice())
    }
}

#[derive(Debug)]
pub struct DiscoverResponder {
    peer: Arc<PeerInner>,
    id: TxId,
}

impl DiscoverResponder {
    pub fn send(self, endpoints: &[StreamInformation]) -> Result<(), Error> {
        let header = SignalingHeader {
            txid: self.id,
            signal_id: SignalIdentifier::Discover,
            message_type: SignalingMessageType::ResponseAccept,
            packet_type: SignalingPacketType::Single,
            num_packets: 1,
        };
        let mut reply = vec![0 as u8; header.size() + endpoints.len() * 2];
        header.encode(&mut reply[0..header.size()])?;
        let mut idx = header.size();
        for endpoint in endpoints {
            endpoint.encode(&mut reply[idx..idx + endpoint.size()])?;
            idx += endpoint.size();
        }
        self.peer.send_signal(reply.as_slice())
    }

    pub fn reject(self, error_code: ErrorCode) -> Result<(), Error> {
        self.peer
            .send_reject(self.id, SignalIdentifier::Discover, error_code)
    }
}

#[derive(Debug)]
pub struct GetCapabilitiesResponder {
    peer: Arc<PeerInner>,
    signal: SignalIdentifier,
    id: TxId,
}

impl GetCapabilitiesResponder {
    pub fn send(self, capabilities: &[ServiceCapability]) -> Result<(), Error> {
        let header = SignalingHeader {
            txid: self.id,
            signal_id: self.signal,
            message_type: SignalingMessageType::ResponseAccept,
            packet_type: SignalingPacketType::Single,
            num_packets: 1,
        };
        let mut reply = vec![
            0 as u8;
            capabilities
                .iter()
                .fold(header.size(), |a, b| a + b.encoded_size())
        ];
        header.encode(&mut reply[0..header.size()])?;
        let mut pos = header.size();
        for capability in capabilities {
            let size = capability.encoded_size();
            capability.encode(&mut reply[pos..pos + size])?;
            pos += size;
        }
        self.peer.send_signal(reply.as_slice())
    }
}

#[derive(Debug)]
struct UnparsedRequest(SignalingHeader, Vec<u8>);

impl UnparsedRequest {
    fn new(header: SignalingHeader, body: Vec<u8>) -> UnparsedRequest {
        UnparsedRequest(header, body)
    }
}

#[derive(Debug, Default)]
struct RequestQueue {
    listener: RequestListener,
    queue: VecDeque<UnparsedRequest>,
}

#[derive(Debug)]
enum RequestListener {
    /// No one is listening.
    None,
    /// Someone wants to listen but hasn't polled.
    New,
    /// Someone is listening, and can be woken whith the waker.
    Some(Waker),
}

impl Default for RequestListener {
    fn default() -> Self {
        RequestListener::None
    }
}

/// An enum representing an interest in the response to a command.
#[derive(Debug)]
enum ResponseWaiter {
    /// A new waiter which hasn't been polled yet.
    WillPoll,
    /// A task waiting for a response, which can be woken with the waker.
    Waiting(Waker),
    /// A response that has been received, stored here until it's polled, at
    /// which point it will be decoded.
    Received(Vec<u8>),
    /// It's still waiting on the reponse, but the receiver has decided they
    /// don't care and we'll throw it out.
    Discard,
}

impl ResponseWaiter {
    /// Check if a message has been received.
    fn is_received(&self) -> bool {
        if let ResponseWaiter::Received(_) = self {
            true
        } else {
            false
        }
    }

    fn unwrap_received(self) -> Vec<u8> {
        if let ResponseWaiter::Received(buf) = self {
            buf
        } else {
            panic!("EXPECTED received buf")
        }
    }
}

fn decode_signaling_response<D: Decodable>(
    expected_signal: SignalIdentifier, buf: Vec<u8>,
) -> Result<D, Error> {
    let header = SignalingHeader::decode(buf.as_slice())?;
    if header.signal_id != expected_signal {
        return Err(Error::InvalidHeader);
    }
    if header.message_type != SignalingMessageType::ResponseAccept {
        return Err(Error::RemoteRejected(buf[header.size()]));
    }
    D::decode(&buf[header.size()..])
}

/// A future that polls for the response to a command we sent.
#[derive(Debug)]
pub struct CommandResponse {
    id: TxId,
    // Some(x) if we're still waiting on the response.
    inner: Option<Arc<PeerInner>>,
}

impl Unpin for CommandResponse {}

impl futures::Future for CommandResponse {
    type Output = Result<Vec<u8>, Error>;
    fn poll(mut self: Pin<&mut Self>, lw: &LocalWaker) -> Poll<Self::Output> {
        let this = &mut *self;
        let res;
        {
            let client = this.inner.as_ref().ok_or(Error::AlreadyReceived)?;
            res = client.poll_recv_response(&this.id, lw);
        }

        if let Poll::Ready(Ok(_)) = res {
            let inner = this
                .inner
                .take()
                .expect("CommandResponse polled after completion");
            inner.wake_any();
        }

        res
    }
}

impl Drop for CommandResponse {
    fn drop(&mut self) {
        if let Some(inner) = &self.inner {
            inner.remove_response_interest(&self.id);
            inner.wake_any();
        }
    }
}

#[derive(Debug)]
struct SignalingHeader {
    txid: TxId,
    packet_type: SignalingPacketType,
    message_type: SignalingMessageType,
    num_packets: u8,
    signal_id: SignalIdentifier,
}

impl Decodable for SignalingHeader {
    fn decode(bytes: &[u8]) -> Result<SignalingHeader, Error> {
        if bytes.len() < 2 {
            return Err(Error::OutOfRange);
        }
        let txid = TxId::try_from(bytes[0] >> 4)?;
        let packet_type = SignalingPacketType::try_from((bytes[0] >> 2) & 0x3)?;
        let (id_offset, num_packets) = match packet_type {
            SignalingPacketType::Start => {
                if bytes.len() < 3 {
                    return Err(Error::OutOfRange);
                }
                (2, bytes[1])
            }
            _ => (1, 1),
        };
        let signal_id_val = bytes[id_offset] & 0x3F;
        let id = SignalIdentifier::try_from(signal_id_val)
            .map_err(|_| Error::InvalidSignalId(txid, signal_id_val))?;
        let header = SignalingHeader {
            txid: TxId::try_from(bytes[0] >> 4)?,
            packet_type: packet_type,
            message_type: SignalingMessageType::try_from(bytes[0] & 0x3)?,
            signal_id: id,
            num_packets: num_packets,
        };
        Ok(header)
    }
}

impl SignalingHeader {
    fn size(&self) -> usize {
        if self.num_packets > 1 {
            3
        } else {
            2
        }
    }

    fn encode(&self, into: &mut [u8]) -> Result<(), Error> {
        if into.len() < 2 {
            return Err(Error::Encoding);
        }
        into[0] = u8::from(&self.txid) << 4
            | u8::from(&self.packet_type) << 2
            | u8::from(&self.message_type);
        into[1] = u8::from(&self.signal_id);
        Ok(())
    }

    fn is_command(&self) -> bool {
        self.message_type == SignalingMessageType::Command
    }
}

#[derive(Debug)]
struct PeerInner {
    /// The signaling channel
    signaling: fasync::Socket,

    /// A map of outstanding message transactions to:
    ///   - None (no response received)
    ///   - Some(AvdtpResponse) (reponse has been received)
    /// Waiters are added with `add_waiter` and get removed when they are
    /// polled or they are removed with `remove_waiter`
    response_waiters: Mutex<Slab<ResponseWaiter>>,

    /// A queue of requests that have been received and are waiting to
    /// be reponded to, along with the waker for the task that has
    /// taken the request receiver (if it exists)
    waiting_requests: Mutex<RequestQueue>,
}

impl PeerInner {
    /// Add a response waiter, and return a id that can be used to send the
    /// transaction.  Responses then can be received using poll_recv_response
    fn add_waiter(&self) -> Result<TxId, Error> {
        let key = self
            .response_waiters
            .lock()
            .insert(ResponseWaiter::WillPoll);
        let id = TxId::try_from(key as u8);
        if id.is_err() {
            self.response_waiters.lock().remove(key);
        }
        id
    }

    /// When a waiter isn't interested in the response anymore, we need to just
    /// throw it out.  This is called when the response future is dropped.
    fn remove_response_interest(&self, id: &TxId) {
        let mut lock = self.response_waiters.lock();
        let idx = usize::from(id);
        if lock[idx].is_received() {
            lock.remove(idx);
        } else {
            lock[idx] = ResponseWaiter::Discard;
        }
    }

    fn poll_recv_request(&self, lw: &LocalWaker) -> Poll<Result<UnparsedRequest, Error>> {
        let is_closed = self.recv_all(lw)?;

        let mut lock = self.waiting_requests.lock();

        if let Some(request) = lock.queue.pop_front() {
            Poll::Ready(Ok(request))
        } else {
            lock.listener = RequestListener::Some(lw.clone().into_waker());
            if is_closed {
                Poll::Ready(Err(Error::PeerDisconnected))
            } else {
                Poll::Pending
            }
        }
    }

    fn poll_recv_response(&self, txid: &TxId, lw: &LocalWaker) -> Poll<Result<Vec<u8>, Error>> {
        let is_closed = self.recv_all(lw)?;

        let mut waiters = self.response_waiters.lock();
        let idx = usize::from(txid);
        if waiters
            .get(idx)
            .expect("Polled unregistered waiter")
            .is_received()
        {
            // We got our response.
            let buf = waiters.remove(idx).unwrap_received();
            Poll::Ready(Ok(buf))
        } else {
            // Set the waker to be notified when a response shows up.
            *waiters.get_mut(idx).expect("Polled unregistered waiter") =
                ResponseWaiter::Waiting(lw.clone().into_waker());

            if is_closed {
                Poll::Ready(Err(Error::PeerDisconnected))
            } else {
                Poll::Pending
            }
        }
    }

    /// Poll for any packets on the signaling socket
    /// Returns whether the channel was closed
    fn recv_all(&self, lw: &LocalWaker) -> Result<bool, Error> {
        let mut buf = Vec::<u8>::new();
        loop {
            let packet_size = match self.signaling.poll_datagram(&mut buf, lw) {
                Poll::Ready(Err(zx::Status::PEER_CLOSED)) => {
                    eprintln!("Peer closed!");
                    return Ok(true);
                }
                Poll::Ready(Err(e)) => return Err(Error::PeerRead(e)),
                Poll::Pending => return Ok(false),
                Poll::Ready(Ok(size)) => size,
            };
            eprintln!("Got data: {:X?}", &buf);
            if packet_size == 0 {
                continue;
            }
            let header = match SignalingHeader::decode(buf.as_slice()) {
                Err(Error::InvalidSignalId(txid, id)) => {
                    self.send_general_reject(txid, id)?;
                    continue;
                }
                Err(_) => Err(Error::InvalidHeader),
                Ok(x) => Ok(x),
            }?;
            // Commands from the remote get translated into requests.
            eprintln!("Got header: {:?}", &header);
            if header.is_command() {
                let mut lock = self.waiting_requests.lock();
                let body = buf.split_off(header.size());
                buf.clear();
                lock.queue.push_back(UnparsedRequest::new(header, body));
                if let RequestListener::Some(ref waker) = lock.listener {
                    waker.wake();
                }
            } else {
                // Should be a response to a command we sent
                let mut waiters = self.response_waiters.lock();
                let idx = usize::from(&header.txid);
                if let Some(&ResponseWaiter::Discard) = waiters.get(idx) {
                    waiters.remove(idx);
                } else if let Some(entry) = waiters.get_mut(idx) {
                    let rest = buf.split_off(packet_size);
                    let old_entry = mem::replace(entry, ResponseWaiter::Received(buf));
                    buf = rest;
                    if let ResponseWaiter::Waiting(waker) = old_entry {
                        waker.wake();
                    }
                }
                // TODO: Respond with error code if we aren't waiting for this txid
            }
        }
    }

    // Wakes up an arbitrary task that has begun polling on the channel so that
    // it will call recv_all and be registered as the new channel reader.
    fn wake_any(&self) {
        // Try to wake up response waiters first, rather than the event listener.
        // The event listener is a stream, and so could be between poll_nexts,
        // Response waiters should always be actively polled once
        // they've begun being polled on a task.
        {
            let lock = self.response_waiters.lock();
            for (_, response_waiter) in lock.iter() {
                if let ResponseWaiter::Waiting(waker) = response_waiter {
                    waker.wake();
                    return;
                }
            }
        }
        {
            let lock = self.waiting_requests.lock();
            if let RequestListener::Some(waker) = &lock.listener {
                waker.wake();
                return;
            }
        }
    }

    fn send_general_reject(&self, txid: TxId, invalid_signal_id: u8) -> Result<(), Error> {
        // We craft the packet ourselves rather than make SignalingHeader
        // build an invalid packet.
        let packet: &[u8; 2] = &[u8::from(&txid) << 4 | 0x01, invalid_signal_id & 0x3F];
        self.send_signal(packet)
    }

    fn send_reject(
        &self, txid: TxId, signal: SignalIdentifier, error_code: ErrorCode,
    ) -> Result<(), Error> {
        let header = SignalingHeader {
            txid: txid,
            signal_id: signal,
            message_type: SignalingMessageType::ResponseReject,
            packet_type: SignalingPacketType::Single,
            num_packets: 1,
        };
        let mut packet = vec![0 as u8; header.size() + 1];
        header.encode(packet.as_mut_slice())?;
        packet[header.size()] = u8::from(&error_code);
        self.send_signal(&packet)
    }

    fn send_signal(&self, data: &[u8]) -> Result<(), Error> {
        self.signaling
            .as_ref()
            .write(data)
            .map_err(|x| Error::PeerWrite(x))?;
        Ok(())
    }
}
