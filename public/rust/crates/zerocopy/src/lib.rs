// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#![no_std]

use core::marker::PhantomData;
use core::mem;
use core::ops::{Deref, DerefMut};

// implement an unsafe trait for all signed and unsigned primitive types
macro_rules! impl_for_primitives {
    ($trait:ident) => (
        impl_for_primitives!(@inner $trait, u8, i8, u16, i16, u32, i32, u64, i64, u128, i128, usize, isize);
    );
    (@inner $trait:ident, $type:ty) => (
        unsafe impl $trait for $type {}
    );
    (@inner $trait:ident, $type:ty, $($types:ty),*) => (
        unsafe impl $trait for $type {}
        impl_for_primitives!(@inner $trait, $($types),*);
    );
}

// implement an unsafe trait for all array lengths up to 32 with an element type
// which implements the trait
macro_rules! impl_for_array_sizes {
    ($trait:ident) => (
        impl_for_array_sizes!(@inner $trait, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32);
    );
    (@inner $trait:ident, $n:expr) => (
        unsafe impl<T: $trait> $trait for [T; $n] {}
    );
    (@inner $trait:ident, $n:expr, $($ns:expr),*) => (
        unsafe impl<T: $trait> $trait for [T; $n] {}
        impl_for_array_sizes!(@inner $trait, $($ns),*);
    );
}

/// Types for which any byte pattern is valid.
///
/// `FromBytes` types can safely be deserialized from an untrusted sequence of
/// bytes because any byte sequence corresponds to a valid instance of the type.
///
/// # Safety
///
/// If `T: FromBytes`, then unsafe code may assume that it is sound to treat any
/// initialized sequence of bytes of length `size_of::<T>()` as a `T`. If a type
/// is marked as `FromBytes` which violates this contract, it may cause
/// undefined behavior.
pub unsafe trait FromBytes {}

/// Types which are safe to treat as an immutable byte slice.
///
/// `AsBytes` types can be safely viewed as a slice of bytes. In particular,
/// this means that, in any valid instance of the type, none of the bytes of the
/// instance are uninitialized. This precludes the following types:
/// - Structs with internal padding
/// - Unions in which not all variants have the same length
///
/// # Safety
///
/// If `T: AsBytes`, then unsafe code may assume that it is sound to treat any
/// instance of the type as an immutable `[u8]` of the appropriate length. If a
/// type is marked as `AsBytes` which violates this contract, it may cause
/// undefined behavior.
pub unsafe trait AsBytes {}

impl_for_primitives!(FromBytes);
impl_for_primitives!(AsBytes);
impl_for_array_sizes!(FromBytes);
impl_for_array_sizes!(AsBytes);

/// Types with no alignment requirement.
///
/// If `T: Unaligned`, then `align_of::<T>() == 1`.
///
/// # Safety
///
/// If `T: Unaligned`, then unsafe code may assume that it is sound to produce a
/// reference to `T` at any memory location regardless of alignment. If a type
/// is marked as `Unaligned` which violates this contract, it may cause
/// undefined behavior.
pub unsafe trait Unaligned {}

unsafe impl Unaligned for u8 {}
unsafe impl Unaligned for i8 {}
impl_for_array_sizes!(Unaligned);

/// A length- and alignment-checked reference to a byte slice which can safely
/// be reinterpreted as another type.
///
/// `LayoutVerified` is either a &[u8] or a &mut [u8] with the invaraint that
/// the slice's length and alignment are each greater than or equal to the
/// length and alignment of `T`. Using this invariant, it implements `Deref` for
/// `T` so long as `T: FromBytes` and `DerefMut` so long as `T: FromBytes +
/// AsBytes`.
///
/// # Examples
///
/// `LayoutVerified` can be used to treat a sequence of bytes as a structured
/// type, and to read and write the fields of that type as if the byte slice
/// reference were simply a reference to that type.
///
/// ```rust
/// use zerocopy::{AsBytes, ByteSlice, FromBytes, LayoutVerified, Unaligned};
///
/// #[repr(C, packed)]
/// struct UdpHeader {
///     src_port: [u8; 2],
///     dst_port: [u8; 2],
///     length: [u8; 2],
///     checksum: [u8; 2],
/// }
///
/// unsafe impl FromBytes for UdpHeader {}
/// unsafe impl AsBytes for UdpHeader {}
/// unsafe impl Unaligned for UdpHeader {}
///
/// struct UdpPacket<B> {
///     header: LayoutVerified<B, UdpHeader>,
///     body: B,
/// }
///
/// impl<B: ByteSlice> UdpPacket<B> {
///     pub fn parse(bytes: B) -> Option<UdpPacket<B>> {
///         let (header, body) = LayoutVerified::new_unaligned_from_prefix(bytes)?;
///         Some(UdpPacket { header, body })
///     }
///
///     pub fn get_src_port(&self) -> [u8; 2] {
///         self.header.src_port
///     }
/// }
///
/// impl<'a> UdpPacket<&'a mut [u8]> {
///     pub fn set_src_port(&mut self, src_port: [u8; 2]) {
///         self.header.src_port = src_port;
///     }
/// }
/// ```
pub struct LayoutVerified<B, T>(B, PhantomData<T>);

impl<B, T> LayoutVerified<B, T>
where
    B: ByteSlice,
    T: Unaligned,
{
    /// Construct a new `LayoutVerified` for a type with no alignment
    /// requirement.
    ///
    /// `new_unaligned` verifies that `bytes.len() >= size_of::<T>()` and
    /// constructs a new `LayoutVerified`. If the check fails, it returns
    /// `None`.
    #[inline]
    pub fn new_unaligned(bytes: B) -> Option<LayoutVerified<B, T>> {
        if bytes.len() < mem::size_of::<T>() {
            return None;
        }
        Some(LayoutVerified(bytes, PhantomData))
    }

    /// Construct a new `LayoutVerified` from the prefix of a byte slice for a
    /// type with no alignment requirement.
    ///
    /// `new_unaligned_from_prefix` verifies that `bytes.len() >=
    /// size_of::<T>()`. It consumes the first `size_of::<T>()` bytes from
    /// `bytes` to construct a `LayoutVerified`, and returns the remaining bytes
    /// to the caller. If the length check fails, it returns `None`.
    #[inline]
    pub fn new_unaligned_from_prefix(bytes: B) -> Option<(LayoutVerified<B, T>, B)> {
        if bytes.len() < mem::size_of::<T>() {
            return None;
        }
        let (bytes, rest) = bytes.split_at(mem::size_of::<T>());
        Some((LayoutVerified(bytes, PhantomData), rest))
    }
}

impl<B, T> LayoutVerified<B, T>
where
    B: ByteSlice,
{
    /// Construct a new `LayoutVerified`.
    ///
    /// `new` verifies that `bytes.len() >= size_of::<T>()` and that `bytes` is
    /// aligned to `align_of::<T>()`, and constructs a new `LayoutVerified`. If
    /// either of these checks fail, it returns `None`.
    #[inline]
    pub fn new(bytes: B) -> Option<LayoutVerified<B, T>> {
        if bytes.len() < mem::size_of::<T>() || !aligned_to(bytes.deref(), mem::align_of::<T>()) {
            return None;
        }
        Some(LayoutVerified(bytes, PhantomData))
    }

    /// Construct a new `LayoutVerified` from the prefix of a byte slice.
    ///
    /// `new_from_prefix` verifies that `bytes.len() >= size_of::<T>()` and that
    /// `bytes` is aligned to `align_of::<T>()`. It consumes the first
    /// `size_of::<T>()` bytes from `bytes` to construct a `LayoutVerified`, and
    /// returns the remaining bytes to the caller. If either the length or
    /// alignment checks fail, it returns `None`.
    #[inline]
    pub fn new_from_prefix(bytes: B) -> Option<(LayoutVerified<B, T>, B)> {
        if bytes.len() < mem::size_of::<T>() || !aligned_to(bytes.deref(), mem::align_of::<T>()) {
            return None;
        }
        let (bytes, rest) = bytes.split_at(mem::size_of::<T>());
        Some((LayoutVerified(bytes, PhantomData), rest))
    }

    #[inline]
    pub fn bytes(&self) -> &[u8] {
        &self.0
    }
}

fn aligned_to(bytes: &[u8], align: usize) -> bool {
    (bytes as *const _ as *const () as usize) % align == 0
}

impl<'a, T> LayoutVerified<&'a mut [u8], T> {
    #[inline]
    pub fn bytes_mut(&mut self) -> &mut [u8] {
        self.0
    }
}

impl<B, T> Deref for LayoutVerified<B, T>
where
    B: ByteSlice,
    T: FromBytes,
{
    type Target = T;
    #[inline]
    fn deref(&self) -> &T {
        unsafe { &*((&self.0) as *const _ as *const T) }
    }
}

impl<'a, T> DerefMut for LayoutVerified<&'a mut [u8], T>
where
    T: FromBytes + AsBytes,
{
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *((&mut self.0) as *mut _ as *mut T) }
    }
}

mod sealed {
    pub trait Sealed {}
}

// ByteSlice abstract over &[u8] and &mut [u8]. We rely on various behaviors of
// [u8] references such as that a given reference will never changes its length
// between calls to deref() or deref_mut(), and that split_at() works as
// expected. If ByteSlice was not sealed, consumers could implement it in a way
// that violated these behaviors, and would break our unsafe code. Thus, we seal
// it and implement it only for [u8] references. For the same reason, it's an
// unsafe trait.

/// `&[u8]` or `&mut [u8]`
///
/// `ByteSlice` abstracts over the mutability of a byte slice reference. It is
/// guaranteed to only be implemented for `&[u8]` and `&mut [u8]`.
pub unsafe trait ByteSlice: Deref<Target = [u8]> + Sized + self::sealed::Sealed {
    fn split_at(self, mid: usize) -> (Self, Self);
}

impl<'a> self::sealed::Sealed for &'a [u8] {}
impl<'a> self::sealed::Sealed for &'a mut [u8] {}
unsafe impl<'a> ByteSlice for &'a [u8] {
    fn split_at(self, mid: usize) -> (Self, Self) {
        <[u8]>::split_at(self, mid)
    }
}
unsafe impl<'a> ByteSlice for &'a mut [u8] {
    fn split_at(self, mid: usize) -> (Self, Self) {
        <[u8]>::split_at_mut(self, mid)
    }
}
