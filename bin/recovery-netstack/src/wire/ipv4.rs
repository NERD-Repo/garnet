// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use byteorder::{BigEndian, ByteOrder};
use zerocopy::{AsBytes, ByteSlice, FromBytes, Unaligned};

use ip::{Ipv4Addr, Ipv4Option};
use wire::util::packet;
use wire::util::packet::{PacketWithOptions, PacketWithOptionsParseErr};
use wire::util::{Checksum, OptionParseErr, Options, PacketFormat};

use self::options::Ipv4OptionImpl;

const HEADER_PREFIX_SIZE: usize = 20;

// HeaderPrefix has the same memory layout (thanks to repr(C, packed)) as an
// IPv4 header. Thus, we can simply reinterpret the bytes of the IPv4 header as
// a HeaderPrefix and then safely access its fields. Note, however, that it is
// *not* safe to have the types of any of the fields be anything other than u8
// or [u8; x] since network byte order (big endian) may not be the same as the
// endianness of the computer we're running on, and since repr(packed) is only
// safe with values with no alignment requirements.
#[repr(C, packed)]
struct HeaderPrefix {
    version_ihl: u8,
    dscp_ecn: u8,
    total_len: [u8; 2],
    id: [u8; 2],
    flags_frag_off: [u8; 2],
    ttl: u8,
    proto: u8,
    hdr_checksum: [u8; 2],
    src_ip: [u8; 4],
    dst_ip: [u8; 4],
}

unsafe impl FromBytes for HeaderPrefix {}
unsafe impl AsBytes for HeaderPrefix {}
unsafe impl Unaligned for HeaderPrefix {}

impl HeaderPrefix {
    fn version(&self) -> u8 {
        self.version_ihl >> 4
    }

    fn ihl(&self) -> u8 {
        self.version_ihl & 0xF
    }
}

impl packet::Header for HeaderPrefix {
    type Error = ();

    fn total_packet_len(&self) -> Result<Option<usize>, ()> {
        Ok(Some(BigEndian::read_u16(&self.total_len[..]) as usize))
    }
}

impl packet::HeaderPrefix for HeaderPrefix {
    fn total_header_len(&self) -> Result<usize, ()> {
        Ok(self.ihl() as usize * 4)
    }
}

/// An IPv4 packet.
///
/// An `Ipv4Packet` shares its underlying memory with the byte slice it was
/// parsed from or serialized to, meaning that no copying or extra allocation is
/// necessary.
// pub struct Ipv4Packet<B> {
//     hdr_prefix: LayoutVerified<B, HeaderPrefix>,
//     options: Options<B, Ipv4OptionImpl>,
//     body: B,
// }

pub struct Ipv4Packet<B>(PacketWithOptions<B, HeaderPrefix, Options<B, Ipv4OptionImpl>>);

impl<B> PacketFormat for Ipv4Packet<B> {
    const MAX_HEADER_BYTES: usize = 60;
    const MAX_FOOTER_BYTES: usize = 0;
}

pub enum Ipv4ParseErr {
    Packet(PacketWithOptionsParseErr<(), OptionParseErr<()>>),
    Version,
    Checksum,
}

impl<B: ByteSlice> Ipv4Packet<B> {
    /// Parse an IPv4 packet.
    ///
    /// `parse` parses `bytes` as an IPv4 packet and validates the checksum.
    #[cfg_attr(feature = "clippy", allow(needless_pass_by_value))]
    pub fn parse(bytes: B) -> Result<Ipv4Packet<B>, Ipv4ParseErr> {
        // See for details: https://en.wikipedia.org/wiki/IPv4#Header

        let packet: PacketWithOptions<B, HeaderPrefix, _> =
            PacketWithOptions::parse(bytes).map_err(Ipv4ParseErr::Packet)?;
        let packet = Ipv4Packet(packet);
        if packet.0.header_prefix().version() != 4 {
            return Err(Ipv4ParseErr::Version);
        }
        if packet.compute_header_checksum() != 0 {
            return Err(Ipv4ParseErr::Checksum);
        }

        Ok(packet)
    }

    pub fn iter_options<'a>(&'a self) -> impl 'a + Iterator<Item = Ipv4Option> {
        self.0.options().iter()
    }
}

impl<B: ByteSlice> Ipv4Packet<B> {
    fn compute_header_checksum(&self) -> u16 {
        let mut c = Checksum::new();
        c.add_bytes(self.0.header_prefix_bytes());
        c.add_bytes(self.0.options().bytes());
        c.sum()
    }

    pub fn body(&self) -> &[u8] {
        self.0.body()
    }

    pub fn version(&self) -> u8 {
        self.0.header_prefix().version()
    }

    pub fn ihl(&self) -> u8 {
        self.0.header_prefix().ihl()
    }

    pub fn dscp(&self) -> u8 {
        self.0.header_prefix().dscp_ecn >> 2
    }

    pub fn ecn(&self) -> u8 {
        self.0.header_prefix().dscp_ecn & 3
    }

    pub fn total_length(&self) -> u16 {
        BigEndian::read_u16(&self.0.header_prefix().total_len)
    }

    pub fn id(&self) -> u16 {
        BigEndian::read_u16(&self.0.header_prefix().id)
    }

    pub fn flags(&self) -> u8 {
        self.0.header_prefix().flags_frag_off[0] >> 5
    }

    pub fn fragment_offset(&self) -> u16 {
        ((u16::from(self.0.header_prefix().flags_frag_off[0] & 0x1F)) << 8)
            | u16::from(self.0.header_prefix().flags_frag_off[1])
    }

    pub fn ttl(&self) -> u8 {
        self.0.header_prefix().ttl
    }

    pub fn proto(&self) -> u8 {
        self.0.header_prefix().proto
    }

    pub fn hdr_checksum(&self) -> u16 {
        BigEndian::read_u16(&self.0.header_prefix().hdr_checksum)
    }

    pub fn src_ip(&self) -> Ipv4Addr {
        Ipv4Addr::new(self.0.header_prefix().src_ip)
    }

    pub fn dst_ip(&self) -> Ipv4Addr {
        Ipv4Addr::new(self.0.header_prefix().dst_ip)
    }
}

impl<'a> Ipv4Packet<&'a mut [u8]> {
    pub fn set_id(&mut self, id: u16) {
        BigEndian::write_u16(&mut self.0.header_prefix_mut().id, id);
    }

    pub fn set_ttl(&mut self, ttl: u8) {
        self.0.header_prefix_mut().ttl = ttl;
    }

    pub fn set_proto(&mut self, proto: u8) {
        self.0.header_prefix_mut().proto = proto;
    }

    pub fn set_src_ip(&mut self, src_ip: Ipv4Addr) {
        self.0.header_prefix_mut().src_ip = src_ip.ipv4_bytes();
    }

    pub fn set_dst_ip(&mut self, dst_ip: Ipv4Addr) {
        self.0.header_prefix_mut().dst_ip = dst_ip.ipv4_bytes();
    }

    /// Compute and set the header checksum.
    ///
    /// Compute the header checksum from the current header state, and set it in
    /// the header.
    pub fn set_checksum(&mut self) {
        self.0.header_prefix_mut().hdr_checksum = [0, 0];
        let c = self.compute_header_checksum();
        BigEndian::write_u16(&mut self.0.header_prefix_mut().hdr_checksum, c);
    }
}

mod options {
    use ip::{Ipv4Option, Ipv4OptionInner};
    use wire::util::OptionImpl;

    const OPTION_KIND_EOL: u8 = 0;
    const OPTION_KIND_NOP: u8 = 1;

    pub struct Ipv4OptionImpl;

    impl OptionImpl for Ipv4OptionImpl {
        type Output = Ipv4Option;
        type Error = ();

        fn parse(kind: u8, data: &[u8]) -> Result<Option<Ipv4Option>, ()> {
            let copied = kind & (1 << 7) > 0;
            match kind {
                self::OPTION_KIND_EOL | self::OPTION_KIND_NOP => {
                    unreachable!("wire::util::Options promises to handle EOL and NOP")
                }
                kind => if data.len() > 38 {
                    Err(())
                } else {
                    let len = data.len();
                    let mut d = [0u8; 38];
                    (&mut d[..len]).copy_from_slice(data);
                    Ok(Some(Ipv4Option {
                        copied,
                        inner: Ipv4OptionInner::Unrecognized {
                            kind,
                            len: len as u8,
                            data: d,
                        },
                    }))
                },
            }
        }
    }
}
