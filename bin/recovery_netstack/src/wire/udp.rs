// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::num::NonZeroU16;

use byteorder::{BigEndian, ByteOrder};
use zerocopy::{AsBytes, ByteSlice, FromBytes, LayoutVerified};

use ip::{Ip, IpAddr, IpProto, IpVersion};
use wire::util::{Checksum, PacketFormat};

const HEADER_SIZE: usize = 8;

// Header has the same memory layout (thanks to repr(C, packed)) as a UDP
// header. Thus, we can simply reinterpret the bytes of the UDP header as a
// Header and then safely access its fields. Note, however, that it is *not*
// safe to have the types of any of the fields be anything other than u8 or [u8;
// x] since network byte order (big endian) may not be the same as the
// endianness of the computer we're running on, and since repr(packed) is only
// safe with values with no alignment requirements.
#[repr(C, packed)]
struct Header {
    src_port: [u8; 2],
    dst_port: [u8; 2],
    length: [u8; 2],
    checksum: [u8; 2],
}

unsafe impl FromBytes for Header {}
unsafe impl AsBytes for Header {}

impl Header {
    fn dst_port(&self) -> u16 {
        BigEndian::read_u16(&self.dst_port)
    }

    fn length(&self) -> u16 {
        BigEndian::read_u16(&self.length)
    }

    fn checksum(&self) -> u16 {
        BigEndian::read_u16(&self.checksum)
    }
}

/// A UDP packet.
///
/// A `UdpPacket` shares its underlying memory with the byte slice it was parsed
/// from or serialized to, meaning that no copying or extra allocation is
/// necessary.
pub struct UdpPacket<B> {
    header: LayoutVerified<B, Header>,
    body: B,
}

impl<B> PacketFormat for UdpPacket<B> {
    const MAX_HEADER_BYTES: usize = 8;
    const MAX_FOOTER_BYTES: usize = 0;
}

impl<B: ByteSlice> UdpPacket<B> {
    /// Parse a UDP packet.
    ///
    /// `parse` parses `bytes` as a UDP packet and validates the checksum.
    ///
    /// `src_ip` is the source address in the IP header. In IPv4, `dst_ip` is
    /// the destination address in the IP header. In IPv6, it's more
    /// complicated. From Wikipedia:
    ///
    /// > The destination address is the final destination; if the IPv6 packet
    /// > does not contain a Routing header, that will be the destination
    /// > address in the IPv6 header; otherwise, at the originating node, it
    /// > will be the address in the last element of the Routing header, and, at
    /// > the receiving node, it will be the destination address in the IPv6
    /// > header.
    pub fn parse<A: IpAddr>(bytes: B, src_ip: A, dst_ip: A) -> Result<UdpPacket<B>, ()> {
        // See for details: https://en.wikipedia.org/wiki/User_Datagram_Protocol#Packet_structure

        let bytes_len = bytes.len();
        let (header, body) = LayoutVerified::<B, Header>::new_from_prefix(bytes).ok_or(())?;
        let packet = UdpPacket { header, body };
        let len = if packet.header.length() == 0 && A::Version::VERSION == IpVersion::V6 {
            // "In IPv6 jumbograms it is possible to have UDP packets of size
            // greater than 65,535 bytes. RFC 2675 specifies that the length
            // field is set to zero if the length of the UDP header plus UDP
            // data is greater than 65,535." - Wikipedia
            if !cfg!(target_pointer_width = "32") && bytes_len >= 1 << 32 {
                // For IPv6, the packet length in the pseudo-header is 32
                // bits. When hdr.length() is used, it fits trivially since
                // hdr.length() is a u16. However, when we use buf.len(), it
                // might overflow on 64-bit platforms. We omit this check on
                // 32-bit platforms because a) buf.len() trivially fits in a
                // u32 and, b) 1 << 32 overflows usize.
                return Err(());
            }
            bytes_len
        } else {
            packet.header.length() as usize
        };
        if len != bytes_len {
            return Err(());
        }
        if packet.header.dst_port() == 0 {
            return Err(());
        }

        // In IPv4, a 0 checksum indicates that the checksum wasn't computed,
        // and so shouldn't be validated.
        if packet.header.checksum != [0, 0] {
            // When computing the checksum, a checksum of 0 is sent as 0xFFFF.
            let target = if packet.header.checksum == [0xFF, 0xFF] {
                0
            } else {
                BigEndian::read_u16(&packet.header.checksum)
            };
            if packet.compute_checksum(src_ip, dst_ip) != target {
                return Err(());
            }
        } else if A::Version::VERSION == IpVersion::V6 {
            //  "Unlike IPv4, when UDP packets are originated by an IPv6 node,
            //  the UDP checksum is not optional.  That is, whenever originating
            //  a UDP packet, an IPv6 node must compute a UDP checksum over the
            //  packet and the pseudo-header, and, if that computation yields a
            //  result of zero, it must be changed to hex FFFF for placement in
            //  the UDP header.  IPv6 receivers must discard UDP packets
            //  containing a zero checksum, and should log the error." - RFC 2460
            return Err(());
        }

        Ok(packet)
    }
}

impl<B: ByteSlice> UdpPacket<B> {
    fn compute_checksum<A: IpAddr>(&self, src_ip: A, dst_ip: A) -> u16 {
        // See for details: https://en.wikipedia.org/wiki/User_Datagram_Protocol#Checksum_computation
        let mut c = Checksum::new();
        c.add_bytes(src_ip.bytes());
        c.add_bytes(dst_ip.bytes());
        if A::Version::VERSION == IpVersion::V4 {
            c.add_bytes(&[0]);
            c.add_bytes(&[IpProto::Udp as u8]);
            c.add_bytes(&self.header.length);
        } else {
            let len = HEADER_SIZE + self.body.len();
            let mut len_bytes = [0; 4];
            BigEndian::write_u32(&mut len_bytes, len as u32);
            c.add_bytes(&len_bytes);
            c.add_bytes(&[0; 3]);
            c.add_bytes(&[IpProto::Udp as u8]);
        }
        c.add_bytes(&self.header.src_port);
        c.add_bytes(&self.header.dst_port);
        c.add_bytes(&self.header.length);
        c.add_bytes(&self.body);
        c.sum()
    }

    pub fn body(&self) -> &[u8] {
        self.body.deref()
    }

    pub fn src_port(&self) -> Option<NonZeroU16> {
        NonZeroU16::new(BigEndian::read_u16(&self.header.src_port))
    }

    pub fn dst_port(&self) -> NonZeroU16 {
        NonZeroU16::new(self.header.dst_port()).unwrap()
    }

    /// Did this packet have a checksum?
    ///
    /// On IPv4, the sender may optionally omit the checksum. If this function
    /// returns false, the sender ommitted the checksum, and `parse` will not
    /// have validated it.
    ///
    /// On IPv6, it is guaranteed that `checksummed` will return true because
    /// IPv6 requires a checksum, and so any UDP packet missing one will fail
    /// validation in `parse`.
    pub fn checksummed(&self) -> bool {
        self.header.checksum() != 0
    }
}
