// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#![deny(warnings)]

use fuchsia_zircon as zx;

pub(crate) trait TryFrom<T>: Sized {
    type Error;
    fn try_from(value: T) -> Result<Self, Self::Error>;
}

macro_rules! decodable_enum {
    ($name:ident<$raw_type:ty> {
        $($variant:ident => $val:expr),*,
    }) => {
        #[derive(Debug, PartialEq)]
        pub(crate) enum $name {
            $($variant),*
        }

        impl From<&$name> for $raw_type {
            fn from(v: &$name) -> $raw_type {
                match v {
                    $($name::$variant => $val),* ,
                }
            }
        }

        impl TryFrom<$raw_type> for $name {
            type Error = Error;
            fn try_from(value: $raw_type) -> Result<Self, Self::Error> {
                match value {
                    $($val => Ok($name::$variant)),* ,
                    _ => Err(Error::OutOfRange),
                }
            }
        }
    }
}

macro_rules! pub_decodable_enum {
    ($(#[$attr:meta])* $name:ident<$raw_type:ty> {
        $($variant:ident => $val:expr),*,
    }) => {
        #[derive(Debug, PartialEq)]
        pub enum $name {
            $($variant),*
        }

        impl From<&$name> for $raw_type {
            fn from(v: &$name) -> $raw_type {
                match v {
                    $($name::$variant => $val),* ,
                }
            }
        }

        impl TryFrom<$raw_type> for $name {
            type Error = Error;
            fn try_from(value: $raw_type) -> Result<Self, Self::Error> {
                match value {
                    $($val => Ok($name::$variant)),* ,
                    _ => Err(Error::OutOfRange),
                }
            }
        }
    }
}

/// An AVDTP Transaction ID.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TxId(u8);

impl TryFrom<u8> for TxId {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        if value > 0xF {
            eprintln!("TxId out of range: {}", value);
            Err(Error::OutOfRange)
        } else {
            Ok(TxId(value))
        }
    }
}

impl From<&TxId> for u8 {
    fn from(v: &TxId) -> u8 {
        v.0
    }
}

impl From<&TxId> for usize {
    fn from(v: &TxId) -> usize {
        v.0 as usize
    }
}

pub_decodable_enum! {
    /// Type of media
    /// Defined in the Bluetooth Assigned Numbers
    /// https://www.bluetooth.com/specifications/assigned-numbers/audio-video
    MediaType<u8> {
        Audio => 0x00,
        Video => 0x01,
        Multimedia => 0x02,
    }
}

pub_decodable_enum! {
    /// Type of endpoint (source or sync)
    EndpointType<u8> {
        Source => 0x00,
        Sink => 0x01,
    }
}

decodable_enum! {
    SignalingPacketType<u8> {
        Single => 0x00,
        Start => 0x01,
        Continue => 0x02,
        End => 0x03,
    }
}

decodable_enum! {
    SignalingMessageType<u8> {
        Command => 0x00,
        GeneralReject => 0x01,
        ResponseAccept => 0x02,
        ResponseReject => 0x03,
    }
}

decodable_enum! {
    SignalIdentifier<u8> {
        Discover => 0x01,
        GetCapabilities => 0x02,
        SetConfiguration => 0x03,
        GetConfiguration => 0x04,
        Reconfigure => 0x05,
        Open => 0x06,
        Start => 0x07,
        Close => 0x08,
        Suspend => 0x09,
        Abort => 0x0A,
        SecurityControl => 0x0B,
        GetAllCapabilities => 0x0C,
        DelayReport => 0x0D,
    }
}

impl Copy for SignalIdentifier {}
impl Clone for SignalIdentifier {
    fn clone(&self) -> SignalIdentifier {
        *self
    }
}

// The error type used by AVDTP operations.
#[derive(Fail, Debug, PartialEq)]
pub enum Error {
    /// The value that eas sent on the wire was out of range.
    #[fail(display = "Value was out of range")]
    OutOfRange,

    /// The signal identifier was invalid when parsing a message.
    #[fail(display = "Invalid signal id for Tx({:?}): {:X?}", _0, _1)]
    InvalidSignalId(TxId, u8),

    /// The header was invalid when parsing a message from the peer.
    #[fail(display = "Invalid Header for a AVDTP message")]
    InvalidHeader,

    /// The header was invalid when parsing a message from the peer.
    #[fail(display = "Failed to parse AVDTP message contents")]
    InvalidMessage,

    /// The request patcket length is not match the assumed length
    #[fail(display = "Command has bad length")]
    BadLength,

    /// The Remote end rejected a command we sent (with this error code)
    #[fail(display = "Remote end rejected the command (code = {:}", _0)]
    RemoteRejected(u8),

    /// Unimplemented
    #[fail(display = "Message has not been implemented yet")]
    UnimplementedMessage,

    /// The distant peer has disconnected.
    #[fail(display = "Peer has disconnected")]
    PeerDisconnected,

    /// Sent if a Command Future is polled after it's already completed
    #[fail(display = "Command Response has already been received")]
    AlreadyReceived,

    /// Encountered an IO error setting up the channel
    #[fail(
        display = "Encountered an IO error reading from the peer: {}",
        _0
    )]
    ChannelSetup(#[cause] zx::Status),

    /// Encountered an IO error reading from the peer.
    #[fail(
        display = "Encountered an IO error reading from the peer: {}",
        _0
    )]
    PeerRead(#[cause] zx::Status),

    /// Encountered an IO error reading from the peer.
    #[fail(
        display = "Encountered an IO error writing to the peer: {}",
        _0
    )]
    PeerWrite(#[cause] zx::Status),

    /// Returened when a message can't be encoded
    #[fail(display = "Encontered an error encoding a message")]
    Encoding,

    #[doc(hidden)]
    #[fail(display = "__Nonexhaustive error should never be created.")]
    __Nonexhaustive,
}

pub_decodable_enum!{
    ErrorCode<u8> {
        BadHeaderFormat => 0x01,
        BadLength => 0x11,
        BadAcpSeid => 0x12,
        SepInUse => 0x13,
        SepNotInUse => 0x14,
        BadServiceCategory => 0x17,
        BadPayloadFormat => 0x18,
        NotSupportedCommand => 0x19,
        InvalidCapabiliies => 0x1A,

        BadRecoveryType => 0x22,
        BadMediaTransportFormat => 0x23,
        BadRecoveryFormat => 0x25,
        BadRohcFormat => 0x26,
        BadCpFormat => 0x27,
        BadMultiplexingFormat => 0x28,
        UnsupportedConfiguration => 0x29,

        BadState => 0x31,
    }
}
