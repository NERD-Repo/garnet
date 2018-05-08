// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::num::NonZeroU16;

use byteorder::{BigEndian, ByteOrder};
use zerocopy::{AsBytes, ByteSlice, FromBytes, LayoutVerified};

use ip::{Ip, IpAddr, IpProto, IpVersion};
use transport::tcp::TcpOption;
use wire::util::{Checksum, Options, PacketFormat};

use self::options::TcpOptionImpl;

const HEADER_PREFIX_SIZE: usize = 20;

// zerocopy!(struct HeaderPrefix {
//     src_port: u16,
//     dst_port: u16,
//     seq_num: u32,
//     ack_num: u32,
//     data_off: u4,
//     reserved: u3,
//     ns: bool,
//     cwr: bool,
//     ece: bool,
//     urg: bool,
//     ack: bool,
//     psh: bool,
//     rst: bool,
//     syn: bool,
//     fin: bool,
//     window_size: u16,
//     checksum: u16,
//     urg_ptr: u16,
// });

// HeaderPrefix has the same memory layout (thanks to repr(C, packed)) as a TCP
// header. Thus, we can simply reinterpret the bytes of the TCP header as a
// HeaderPrefix and then safely access its fields. Note, however, that it is
// *not* safe to have the types of any of the fields be anything other than u8
// or [u8; x] since network byte order (big endian) may not be the same as the
// endianness of the computer we're running on, and since repr(packed) is only
// safe with values with no alignment requirements.
#[repr(C, packed)]
struct HeaderPrefix {
    src_port: [u8; 2],
    dst_port: [u8; 2],
    seq_num: [u8; 4],
    ack: [u8; 4],
    data_off_reserved_ns: u8,
    flags: u8,
    window_size: [u8; 2],
    checksum: [u8; 2],
    urg_ptr: [u8; 2],
}

unsafe impl FromBytes for HeaderPrefix {}
unsafe impl AsBytes for HeaderPrefix {}

impl HeaderPrefix {
    pub fn src_port(&self) -> u16 {
        BigEndian::read_u16(&self.src_port)
    }

    pub fn dst_port(&self) -> u16 {
        BigEndian::read_u16(&self.dst_port)
    }

    fn data_off(&self) -> u8 {
        self.data_off_reserved_ns >> 4
    }
}

/// A TCP segment.
///
/// A `TcpSegment` shares its underlying memory with the byte slice it was
/// parsed from or serialized to, meaning that no copying or extra allocation is
/// necessary.
pub struct TcpSegment<B> {
    hdr_prefix: LayoutVerified<B, HeaderPrefix>,
    options: Options<B, TcpOptionImpl>,
    body: B,
}

impl<B> PacketFormat for TcpSegment<B> {
    const MAX_HEADER_BYTES: usize = 60;
    const MAX_FOOTER_BYTES: usize = 0;
}

impl<B: ByteSlice> TcpSegment<B> {
    /// Parse a TCP segment.
    ///
    /// `parse` parses `bytes` as a TCP segment and validates the checksum.
    #[cfg_attr(feature = "clippy", allow(needless_pass_by_value))]
    pub fn parse<A: IpAddr>(bytes: B, src_ip: A, dst_ip: A) -> Result<TcpSegment<B>, ()> {
        // See for details: https://en.wikipedia.org/wiki/Transmission_Control_Protocol#TCP_segment_structure

        let total_len = bytes.len();
        let (hdr_prefix, rest) =
            LayoutVerified::<B, HeaderPrefix>::new_from_prefix(bytes).ok_or((()))?;
        let hdr_bytes = (hdr_prefix.data_off() * 4) as usize;
        if hdr_bytes > HEADER_PREFIX_SIZE + rest.len() {
            return Err(());
        }
        let (options, body) = rest.split_at(hdr_bytes - HEADER_PREFIX_SIZE);
        let options = Options::parse(options).map_err(|_| ())?;
        let segment = TcpSegment {
            hdr_prefix,
            options,
            body,
        };

        // For IPv4, the "TCP length" field in the pseudo-header used for
        // calculating checksums is 16 bits. For IPv6, it's 32 bits. Verify that
        // the length of the entire payload (including header) does not
        // overflow. On 32-bit platforms for IPv6, we omit the check since '1 <<
        // 32' would overflow usize.
        if A::Version::VERSION == IpVersion::V4 && total_len >= 1 << 16
            || (!cfg!(target_pointer_width = "32") && total_len >= 1 << 32)
        {
            return Err(());
        }
        if segment.compute_checksum(src_ip, dst_ip) != 0 {
            return Err(());
        }
        Ok(segment)
    }

    pub fn iter_options<'a>(&'a self) -> impl 'a + Iterator<Item = TcpOption> {
        self.options.iter()
    }
}

impl<B: ByteSlice> TcpSegment<B> {
    fn compute_checksum<A: IpAddr>(&self, src_ip: A, dst_ip: A) -> u16 {
        // See for details: https://en.wikipedia.org/wiki/Transmission_Control_Protocol#Checksum_computation
        let mut checksum = Checksum::new();
        checksum.add_bytes(src_ip.bytes());
        checksum.add_bytes(dst_ip.bytes());
        let total_len =
            self.hdr_prefix.bytes().len() + self.options.bytes().len() + self.body.len();
        if A::Version::VERSION == IpVersion::V4 {
            checksum.add_bytes(&[0]);
            checksum.add_bytes(&[IpProto::Tcp as u8]);
            // For IPv4, the "TCP length" field in the pseudo-header is 16 bits.
            let mut l = [0; 2];
            BigEndian::write_u16(&mut l, total_len as u16);
            checksum.add_bytes(&l);
        } else {
            // For IPv6, the "TCP length" field in the pseudo-header is 32 bits.
            let mut l = [0; 4];
            BigEndian::write_u32(&mut l, total_len as u32);
            checksum.add_bytes(&l);
            checksum.add_bytes(&[0; 3]);
            checksum.add_bytes(&[IpProto::Tcp as u8])
        }
        checksum.add_bytes(self.hdr_prefix.bytes());
        checksum.add_bytes(self.options.bytes());
        checksum.add_bytes(&self.body);
        checksum.sum()
    }

    pub fn body(&self) -> &[u8] {
        &self.body
    }

    pub fn src_port(&self) -> NonZeroU16 {
        NonZeroU16::new(self.hdr_prefix.src_port()).unwrap()
    }

    pub fn dst_port(&self) -> NonZeroU16 {
        NonZeroU16::new(self.hdr_prefix.dst_port()).unwrap()
    }

    pub fn seq_num(&self) -> u32 {
        // self.hdr_prefix.get_seq_num()
        BigEndian::read_u32(&self.hdr_prefix.seq_num)
    }

    fn get_flag(&self, mask: u8) -> bool {
        self.hdr_prefix.flags & mask > 0
    }

    pub fn ack_num(&self) -> u32 {
        // self.hdr_prefix.get_ack_num()
        BigEndian::read_u32(&self.hdr_prefix.ack)
    }

    pub fn ack(&self) -> bool {
        // self.hdr_prefix.get_ack()
        self.get_flag(ACK_MASK)
    }

    pub fn rst(&self) -> bool {
        // self.hdr_prefix.get_rst()
        self.get_flag(RST_MASK)
    }

    pub fn syn(&self) -> bool {
        // self.hdr_prefix.get_syn()
        self.get_flag(SYN_MASK)
    }

    pub fn fin(&self) -> bool {
        // self.hdr_prefix.get_fin()
        self.get_flag(FIN_MASK)
    }

    pub fn window_size(&self) -> u16 {
        // self.hdr_prefix.get_window_size()
        BigEndian::read_u16(&self.hdr_prefix.window_size)
    }
}

impl<'a> TcpSegment<&'a mut [u8]> {
    pub fn set_src_port(&mut self, src_port: NonZeroU16) {
        // self.hdr_prefix.set_src_port(src_port.get());
        BigEndian::write_u16(&mut self.hdr_prefix.src_port, src_port.get());
    }

    pub fn set_dst_port(&mut self, dst_port: NonZeroU16) {
        // self.hdr_prefix.set_src_port(dst_port.get());
        BigEndian::write_u16(&mut self.hdr_prefix.dst_port, dst_port.get());
    }

    pub fn set_seq_num(&mut self, seq_num: u32) {
        // self.hdr_prefix.set_seq_num(seq_num);
        BigEndian::write_u32(&mut self.hdr_prefix.seq_num, seq_num);
    }

    pub fn set_ack_num(&mut self, ack_num: u32) {
        // self.hdr_prefix.set_seq_num(ack_num);
        BigEndian::write_u32(&mut self.hdr_prefix.ack, ack_num);
    }

    fn set_flag(&mut self, mask: u8, set: bool) {
        if set {
            self.hdr_prefix.flags |= mask;
        } else {
            self.hdr_prefix.flags &= 0xFF - mask;
        }
    }

    pub fn set_ack(&mut self, ack: bool) {
        // self.hdr_prefix.set_ack(ack);
        self.set_flag(ACK_MASK, ack);
    }

    pub fn set_rst(&mut self, rst: bool) {
        // self.hdr_prefix.set_rst(rst);
        self.set_flag(RST_MASK, rst);
    }

    pub fn set_syn(&mut self, syn: bool) {
        // self.hdr_prefix.set_syn(syn);
        self.set_flag(SYN_MASK, syn);
    }

    pub fn set_fin(&mut self, fin: bool) {
        // self.hdr_prefix.set_fin(fin);
        self.set_flag(FIN_MASK, fin);
    }

    pub fn set_window_size(&mut self, window_size: u16) {
        // self.hdr_prefix.set_window_size(window_size);
        BigEndian::write_u16(&mut self.hdr_prefix.window_size, window_size);
    }

    /// Compute and set the TCP checksum.
    ///
    /// Compute the TCP checksum from the current segment state, source IP, and
    /// destination IP, and set it in the header.
    pub fn set_checksum<A: IpAddr>(&mut self, src_ip: A, dst_ip: A) {
        // self.hdr_prefix.set_checksum(0);
        self.hdr_prefix.checksum = [0, 0];
        let c = self.compute_checksum(src_ip, dst_ip);
        // self.hdr_prefix.set_checksum(c);
        BigEndian::write_u16(&mut self.hdr_prefix.checksum, c);
    }
}

const ACK_MASK: u8 = 1u8 << 4;
const RST_MASK: u8 = 1u8 << 2;
const SYN_MASK: u8 = 1u8 << 1;
const FIN_MASK: u8 = 1u8;

mod options {
    use std::mem;

    use byteorder::{BigEndian, ByteOrder};

    use transport::tcp::{TcpOption, TcpSackBlock};
    use wire::util::OptionImpl;

    fn parse_sack_block(bytes: &[u8]) -> TcpSackBlock {
        TcpSackBlock {
            left_edge: BigEndian::read_u32(bytes),
            right_edge: BigEndian::read_u32(&bytes[4..]),
        }
    }

    const OPTION_KIND_EOL: u8 = 0;
    const OPTION_KIND_NOP: u8 = 1;
    const OPTION_KIND_MSS: u8 = 2;
    const OPTION_KIND_WINDOW_SCALE: u8 = 3;
    const OPTION_KIND_SACK_PERMITTED: u8 = 4;
    const OPTION_KIND_SACK: u8 = 5;
    const OPTION_KIND_TIMESTAMP: u8 = 8;

    pub struct TcpOptionImpl;

    impl OptionImpl for TcpOptionImpl {
        type Output = TcpOption;
        type Error = ();

        fn parse(kind: u8, data: &[u8]) -> Result<Option<TcpOption>, ()> {
            match kind {
                self::OPTION_KIND_EOL | self::OPTION_KIND_NOP => {
                    unreachable!("wire::util::Options promises to handle EOL and NOP")
                }
                self::OPTION_KIND_MSS => if data.len() != 2 {
                    Err(())
                } else {
                    Ok(Some(TcpOption::Mss(BigEndian::read_u16(&data))))
                },
                self::OPTION_KIND_WINDOW_SCALE => if data.len() != 1 {
                    Err(())
                } else {
                    Ok(Some(TcpOption::WindowScale(data[0])))
                },
                self::OPTION_KIND_SACK_PERMITTED => if !data.is_empty() {
                    Err(())
                } else {
                    Ok(Some(TcpOption::SackPermitted))
                },
                self::OPTION_KIND_SACK => match data.len() {
                    8 | 16 | 24 | 32 => {
                        let num_blocks = data.len() / mem::size_of::<TcpSackBlock>();
                        let mut blocks = [TcpSackBlock::default(); 4];
                        for i in 0..num_blocks {
                            blocks[i] = parse_sack_block(&data[i * 8..]);
                        }
                        Ok(Some(TcpOption::Sack {
                            blocks,
                            num_blocks: num_blocks as u8,
                        }))
                    }
                    _ => Err(()),
                },
                self::OPTION_KIND_TIMESTAMP => if data.len() != 8 {
                    Err(())
                } else {
                    let ts_val = BigEndian::read_u32(&data);
                    let ts_echo_reply = BigEndian::read_u32(&data[4..]);
                    Ok(Some(TcpOption::Timestamp {
                        ts_val,
                        ts_echo_reply,
                    }))
                },
                _ => Ok(None),
            }
        }
    }
}
