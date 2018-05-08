// Copyright 2018 The Fuchsia Authors. All rights reserved. Use of this source
// code is governed by a BSD-style license that can be found in the LICENSE
// file.

use byteorder::{BigEndian, ByteOrder};
use zerocopy::{AsBytes, ByteSlice, FromBytes, LayoutVerified};

use device::ethernet::MAC;
use wire::util::PacketFormat;

// Header has the same memory layout (thanks to repr(C, packed)) as an Ethernet
// header prefix. Thus, we can simply reinterpret the bytes of the Ethernet
// header prefix as a HeaderPrefix and then safely access its fields. Note,
// however, that it is *not* safe to have the types of any of the fields be
// anything other than u8 or [u8; x] since network byte order (big endian) may
// not be the same as the endianness of the computer we're running on, and since
// repr(packed) is only safe with values with no alignment requirements.
#[repr(C, packed)]
struct HeaderPrefix {
    dst_mac: [u8; 6],
    src_mac: [u8; 6],
}

unsafe impl FromBytes for HeaderPrefix {}
unsafe impl AsBytes for HeaderPrefix {}

const TPID_8021Q: u16 = 0x8100;
const TPID_8021AD: u16 = 0x88a8;

enum Tag {
    Tag8021Q(u16),
    Tag8021ad(u16),
    None,
}

/// An Ethernet frame.
///
/// An `EthernetFrame` shares its underlying memory with the byte slice it was
/// parsed from or serialized to, meaning that no copying or extra allocation is
/// necessary.
pub struct EthernetFrame<B> {
    hdr_prefix: LayoutVerified<B, HeaderPrefix>,
    tag: Tag,
    ethertype: u16,
    body: B,
}

impl<B> PacketFormat for EthernetFrame<B> {
    const MAX_HEADER_BYTES: usize = 18;
    const MAX_FOOTER_BYTES: usize = 0;
}

impl<B: ByteSlice> EthernetFrame<B> {
    /// Parse an Ethernet frame.
    ///
    /// `parse` parses `bytes` as an Ethernet frame.
    pub fn parse(bytes: B) -> Result<EthernetFrame<B>, ()> {
        // See for details: https://en.wikipedia.org/wiki/Ethernet_frame#Frame_%E2%80%93_data_link_layer

        let (hdr_prefix, rest) =
            LayoutVerified::<B, HeaderPrefix>::new_from_prefix(bytes).ok_or(())?;
        if rest.len() < 46 {
            // "The minimum payload is 42 octets when an 802.1Q tag is present
            // and 46 octets when absent." - Wikipedia
            //
            // An 802.1Q tag is 4 bytes, and we haven't consumed it yet, so in
            // either case, the minimum is 46.
            return Err(());
        }

        // "The IEEE 802.1Q tag or IEEE 802.1ad tag, if present, is a four-octet
        // field that indicates virtual LAN (VLAN) membership and IEEE 802.1p
        // priority. The first two octets of the tag are called the Tag Protocol
        // IDentifier and double as the EtherType field indicating that the
        // frame is either 802.1Q or 802.1ad tagged. 802.1Q uses a TPID of
        // 0x8100. 802.1ad uses a TPID of 0x88a8." - Wikipedia
        let ethertype = BigEndian::read_u16(&rest);
        // in case a tag is present; if not, these are the first two bytes of
        // the payload, and we don't use this variable
        let next_u16 = BigEndian::read_u16(&rest[2..]);
        let (tag, ethertype, body) = match ethertype {
            self::TPID_8021Q => {
                let (ethertype, body) = rest.split_at(2);
                (
                    Tag::Tag8021Q(next_u16),
                    BigEndian::read_u16(&ethertype),
                    body,
                )
            }
            self::TPID_8021AD => {
                let (ethertype, body) = rest.split_at(2);
                (
                    Tag::Tag8021ad(next_u16),
                    BigEndian::read_u16(&ethertype),
                    body,
                )
            }
            ethertype => (Tag::None, ethertype, rest),
        };

        Ok(EthernetFrame {
            hdr_prefix,
            tag,
            ethertype,
            body,
        })
    }
}

impl<B: ByteSlice> EthernetFrame<B> {
    pub fn src_mac(&self) -> MAC {
        MAC::new(self.hdr_prefix.src_mac)
    }

    pub fn dst_mac(&self) -> MAC {
        MAC::new(self.hdr_prefix.dst_mac)
    }

    pub fn ethertype(&self) -> u16 {
        self.ethertype
    }
}
