pub use self::checksum::*;
pub use self::options::*;
pub use self::serialize::*;

mod checksum {
    use byteorder::{BigEndian, ByteOrder};

    /// A checksum used by IPv4 and TCP.
    ///
    /// This checksum operates by computing the 1s complement sum of successive
    /// 16-bit words of the input.
    pub struct Checksum(u32);

    impl Checksum {
        /// Initialize a new checksum.
        pub fn new() -> Self {
            Checksum(0)
        }

        /// Add bytes to the checksum.
        ///
        /// If `bytes` does not contain an even number of bytes, a single zero byte
        /// will be added to the end before updating the checksum.
        pub fn add_bytes(&mut self, mut bytes: &[u8]) {
            while bytes.len() > 1 {
                self.0 += u32::from(BigEndian::read_u16(bytes));
                bytes = &bytes[2..];
            }
            if bytes.len() == 1 {
                self.0 += u32::from(BigEndian::read_u16(&[bytes[0], 0]));
            }
        }

        /// Compute the checksum.
        ///
        /// `sum` returns the checksum of all data added using `add_bytes` so far.
        /// Calling `sum` does *not* reset the checksum. More bytes may be added
        /// after calling `sum`, and they will be added to the checksum as expected.
        pub fn sum(&self) -> u16 {
            let mut sum = self.0;
            while (sum >> 16) != 0 {
                sum = (sum >> 16) + (sum & 0xFF);
            }
            !sum as u16
        }
    }

    /// Checksum bytes.
    ///
    /// `checksum` is a shorthand for
    ///
    /// ```rust
    /// let mut c = Checksum::new();
    /// c.add_bytes(bytes);
    /// c.sum()
    /// ```
    pub fn checksum(bytes: &[u8]) -> u16 {
        let mut c = Checksum::new();
        c.add_bytes(bytes);
        c.sum()
    }
}

mod options {
    use std::fmt::Debug;
    use std::marker::PhantomData;
    use std::ops::Deref;

    use zerocopy::ByteSlice;

    /// A parsed set of header options.
    ///
    /// `Options` represents a parsed set of options from a TCP or IPv4 header.
    pub struct Options<B, O> {
        bytes: B,
        _marker: PhantomData<O>,
    }

    /// An iterator over header options.
    ///
    /// `OptionIter` is an iterator over packet header options stored in the
    /// format used by IPv4 and TCP, where each option is either a single kind
    /// byte or a kind byte, a length byte, and length - 2 data bytes.
    ///
    /// In both IPv4 and TCP, the only single-byte options are End of Options
    /// List (EOL) and No Operation (NOP), both of which can be handled
    /// internally by OptionIter. Thus, the caller only needs to be able to
    /// parse multi-byte options.
    pub struct OptionIter<'a, O> {
        bytes: &'a [u8],
        idx: usize,
        _marker: PhantomData<O>,
    }

    /// Errors returned from parsing options.
    ///
    /// `OptionParseErr` is either `Internal`, which indicates that this module
    /// encountered a malformed sequence of options (likely with a length field
    /// larger than the remaining bytes in the options buffer), or `External`,
    /// which indicates that the `OptionImpl::parse` callback returned an error.
    #[derive(Debug)]
    pub enum OptionParseErr<E> {
        Internal,
        External(E),
    }

    /// An implementation of an options parser.
    ///
    /// `OptionImpl` provides functions to parse fixed- and variable-length
    /// options. It is required in order to construct an `Options` or
    /// `OptionIter`.
    pub trait OptionImpl {
        type Output;
        type Error;

        /// Parse an option.
        ///
        /// `parse` takes a kind byte and variable-length data associated and
        /// returns `Ok(Some(o))` if the option successfully parsed as `o`,
        /// `Ok(None)` if the kind byte was unrecognized, and `Err(err)` if the
        /// kind byte was recognized but `data` was malformed for that option
        /// kind. `parse` is allowed to not recognize certain option kinds, as
        /// the length field can still be used to safely skip over them.
        ///
        /// `parse` must be deterministic, or else `Options::parse` cannot
        /// guarantee that future iterations will not produce errors (and
        /// panic).
        fn parse(kind: u8, data: &[u8]) -> Result<Option<Self::Output>, Self::Error>;
    }

    impl<B, O> Options<B, O>
    where
        B: ByteSlice,
        O: OptionImpl,
    {
        /// Parse a set of options.
        ///
        /// `parse` parses `bytes` as a sequence of options. `parse` performs a
        /// single pass over all of the options to verify that they are
        /// well-formed. Once `parse` returns successfully, the resulting
        /// `Options` can be used to construct infallible iterators.
        pub fn parse(bytes: B) -> Result<Options<B, O>, OptionParseErr<O::Error>> {
            // First, do a single pass over the bytes to detect any errors up
            // front. Once this is done, since we have a reference to `bytes`,
            // these bytes can't change out from under us, and so we can treat
            // any iterator over these bytes as infallible. This makes a few
            // assumptions, but none of them are that big of a deal. In all
            // cases, breaking these assumptions would just result in a runtime
            // panic.
            // - B could return different bytes each time
            // - O::parse could be non-deterministic
            while next::<B, O>(&bytes, &mut 0)?.is_some() {}
            Ok(Options {
                bytes,
                _marker: PhantomData,
            })
        }
    }

    impl<B: Deref<Target = [u8]>, O> Options<B, O> {
        /// Get the underlying bytes.
        ///
        /// `bytes` returns a reference to the byte slice backing this
        /// `Options`.
        pub fn bytes(&self) -> &[u8] {
            &self.bytes
        }
    }

    impl<'a, B, O> Options<B, O>
    where
        B: 'a + ByteSlice,
        O: OptionImpl,
    {
        /// Create an iterator over options.
        ///
        /// `iter` constructs an iterator over the options. Since the options
        /// were validated in `parse`, then so long as `from_kind` and
        /// `from_data` are deterministic, the iterator is infallible.
        pub fn iter(&'a self) -> OptionIter<'a, O> {
            OptionIter {
                bytes: &self.bytes,
                idx: 0,
                _marker: PhantomData,
            }
        }
    }

    impl<'a, O> Iterator for OptionIter<'a, O>
    where
        O: OptionImpl,
        O::Error: Debug,
    {
        type Item = O::Output;

        fn next(&mut self) -> Option<O::Output> {
            next::<&'a [u8], O>(&self.bytes, &mut self.idx)
                .expect("already-validated options should not fail to parse")
        }
    }

    // End of Options List in both IPv4 and TCP
    const END_OF_OPTIONS: u8 = 0;
    // NOP in both IPv4 and TCP
    const NOP: u8 = 1;

    fn next<B, O>(bytes: &B, idx: &mut usize) -> Result<Option<O::Output>, OptionParseErr<O::Error>>
    where
        B: ByteSlice,
        O: OptionImpl,
    {
        // For an explanation of this format, see the "Options" section of
        // https://en.wikipedia.org/wiki/Transmission_Control_Protocol#TCP_segment_structure
        loop {
            let bytes = &bytes[*idx..];
            if bytes.is_empty() {
                return Ok(None);
            }
            if bytes[0] == END_OF_OPTIONS {
                return Ok(None);
            }
            if bytes[0] == NOP {
                *idx += 1;
                continue;
            }
            let len = bytes[1] as usize;
            if len < 2 || len > bytes.len() {
                return Err(OptionParseErr::Internal);
            }
            *idx += len;
            match O::parse(bytes[0], &bytes[2..]) {
                Ok(Some(o)) => return Ok(Some(o)),
                Ok(None) => {}
                Err(err) => return Err(OptionParseErr::External(err)),
            }
        }
    }

    impl<'a, B, O> super::packet::Options<B> for Options<B, O>
    where
        B: 'a + ByteSlice,
        O: OptionImpl,
    {
        type Error = OptionParseErr<O::Error>;

        fn from_bytes(bytes: B) -> Result<Self, Self::Error> {
            Options::parse(bytes)
        }
    }
}

pub mod packet {
    use zerocopy::{AsBytes, ByteSlice, FromBytes, LayoutVerified, Unaligned};

    // NOTE(joshlf): We used Unaligned for Packet*::parse because, if we didn't,
    // the caller would be responsible for ensuring that the packet buffers
    // receiver were properly aligned. That would make the system much more
    // complicated for a number of reasons, not least of which is that if a
    // given packet is aligned, and then a header is stripped off, unless that
    // header is a sufficiently-large power of two (which most of our headers
    // aren't), the inner packet body will not be aligned. If that inner packet
    // body is itself another packet with a header with alignment requirements,
    // it will likely be unaligned. Much easier to simply require Unaligned
    // headers.

    /// A packet header.
    ///
    /// A `Header` represents a packet header which can optionally encode the
    /// total length of the packet that it precedes. If it does, then
    /// `Packet::parse` and `PacketWithOptions::parse` will validate the packet
    /// against this length.
    pub trait Header {
        /// Errors returned from `total_packet_length`.
        type Error;

        // total packet length including header; if None, body is simply taken
        // to be whatever's left after header and options

        /// The total length of this packet in bytes.
        ///
        /// `total_packet_len` returns the total length of the packet in bytes
        /// as encoded in this header, or a parsing error. If no length is
        /// encoded, it returns `Ok(None)`.
        fn total_packet_len(&self) -> Result<Option<usize>, Self::Error>;
    }

    /// The prefix of a header with variable-length options.
    ///
    /// A `HeaderPrefix` is a `Header` which is followed by a variable-length
    /// options field before the packet body.
    pub trait HeaderPrefix: Header {
        /// The total length of this header - including the options - in bytes.
        ///
        /// `total_header_len` returns the number of bytes consumed by both this
        /// header prefix and the variable-length options. All bytes following
        /// are part of the body.
        fn total_header_len(&self) -> Result<usize, Self::Error>;
    }

    /// Errors returned from `Packet::parse`.
    pub enum PacketParseErr<E> {
        Packet,
        Header(E),
    }

    /// A parsed packet.
    ///
    /// A `Packet` represents a parsed packet backed by byte slice `B` and header type `H`.
    pub struct Packet<B, H> {
        header: LayoutVerified<B, H>,
        body: B,
    }

    impl<B, H> Packet<B, H>
    where
        B: ByteSlice,
        H: Header + FromBytes + Unaligned,
    {
        pub fn parse(bytes: B) -> Result<Packet<B, H>, PacketParseErr<H::Error>> {
            let total_len = bytes.len();
            let (header, body): (LayoutVerified<B, H>, B) =
                LayoutVerified::new_unaligned_from_prefix(bytes).ok_or(PacketParseErr::Packet)?;
            if let Some(len) = header
                .total_packet_len()
                .map_err(|err| PacketParseErr::Header(err))?
            {
                if len != total_len {
                    return Err(PacketParseErr::Packet);
                }
            }
            Ok(Packet { header, body })
        }
    }

    impl<B, H> Packet<B, H>
    where
        B: ByteSlice,
    {
        pub fn body(&self) -> &[u8] {
            &self.body
        }
    }

    impl<B, H> Packet<B, H>
    where
        B: ByteSlice,
        H: FromBytes,
    {
        pub fn header(&self) -> &H {
            &self.header
        }
    }

    impl<'a, H> Packet<&'a mut [u8], H> {
        pub fn body_mut(&mut self) -> &mut [u8] {
            &mut self.body
        }
    }

    impl<'a, H> Packet<&'a mut [u8], H>
    where
        H: FromBytes + AsBytes,
    {
        pub fn header_mut(&mut self) -> &mut H {
            &mut self.header
        }
    }

    pub trait Options<B: ByteSlice>
    where
        Self: Sized,
    {
        type Error;

        fn from_bytes(b: B) -> Result<Self, Self::Error>;
    }

    pub struct PacketWithOptions<B, H, O> {
        header_prefix: LayoutVerified<B, H>,
        options: O,
        body: B,
    }

    pub enum PacketWithOptionsParseErr<E, F> {
        Packet,
        Header(E),
        Options(F),
    }

    impl<B, H, O> PacketWithOptions<B, H, O>
    where
        B: ByteSlice,
        H: HeaderPrefix + FromBytes + Unaligned,
        O: Options<B>,
    {
        pub fn parse(
            bytes: B,
        ) -> Result<PacketWithOptions<B, H, O>, PacketWithOptionsParseErr<H::Error, O::Error>>
        {
            let total_len = bytes.len();
            let (header_prefix, rest): (LayoutVerified<B, H>, B) =
                LayoutVerified::new_unaligned_from_prefix(bytes)
                    .ok_or(PacketWithOptionsParseErr::Packet)?;
            if let Some(len) = header_prefix
                .total_packet_len()
                .map_err(|err| PacketWithOptionsParseErr::Header(err))?
            {
                if len != total_len {
                    return Err(PacketWithOptionsParseErr::Packet);
                }
            }
            let header_prefix_len = total_len - rest.len();
            let total_header_len = header_prefix
                .total_header_len()
                .map_err(|err| PacketWithOptionsParseErr::Header(err))?;
            let options_len = if total_header_len < header_prefix_len {
                return Err(PacketWithOptionsParseErr::Packet);
            } else {
                total_header_len - header_prefix_len
            };
            let (options, body) = rest.split_at(options_len);
            let options: O =
                O::from_bytes(options).map_err(|err| PacketWithOptionsParseErr::Options(err))?;
            Ok(PacketWithOptions {
                header_prefix,
                options,
                body,
            })
        }
    }

    impl<B, H, O> PacketWithOptions<B, H, O> {
        pub fn options(&self) -> &O {
            &self.options
        }

        pub fn options_mut(&mut self) -> &mut O {
            &mut self.options
        }
    }

    impl<B, H, O> PacketWithOptions<B, H, O>
    where
        B: ByteSlice,
    {
        pub fn header_prefix_bytes(&self) -> &[u8] {
            self.header_prefix.bytes()
        }

        pub fn body(&self) -> &[u8] {
            &self.body
        }
    }

    impl<B, H, O> PacketWithOptions<B, H, O>
    where
        B: ByteSlice,
        H: FromBytes,
    {
        pub fn header_prefix(&self) -> &H {
            &self.header_prefix
        }
    }

    impl<'a, H, O> PacketWithOptions<&'a mut [u8], H, O> {
        pub fn header_prefix_bytes_mut(&mut self) -> &mut [u8] {
            self.header_prefix.bytes_mut()
        }

        pub fn body_mut(&mut self) -> &mut [u8] {
            &mut self.body
        }
    }

    impl<'a, H, O> PacketWithOptions<&'a mut [u8], H, O>
    where
        H: FromBytes + AsBytes,
    {
        pub fn header_prefix_mut(&mut self) -> &mut H {
            &mut self.header_prefix
        }
    }
}

mod serialize {
    pub trait PacketFormat {
        /// The maximum length of a packet header in bytes.
        ///
        /// If `MAX_HEADER_BYTES` bytes are allocated in a buffer preceding a
        /// payload, it is guaranteed that any header generated by this packet
        /// format will be able to fit in the space preceding the payload.
        const MAX_HEADER_BYTES: usize;

        /// The maximum length of a packet footer in bytes.
        ///
        /// If `MAX_FOOTER_BYTES` bytes are allocated in a buffer following a
        /// payload, it is guaranteed that any footer generated by this packet
        /// format will be able to fit in the space following the payload.
        const MAX_FOOTER_BYTES: usize;
    }
}
