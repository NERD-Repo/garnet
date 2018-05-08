#![no_std]

use core::marker::PhantomData;
use core::mem;
use core::ops::{Deref, DerefMut};
use core::ptr;

// TODO:
// - FromBits
//   - Is it safe to relax the constraint when T is a DST to say that the
//     conversion is valid so long as size_of_val(t) > size_of::<Self>()?
//   - Figure out what to do when Self is a DST.
// - transmute
//   - Add various ref/mut implementations?

/// Types which can be constructed from the bits of a `T`.
///
/// `FromBits<T>` is a marker trait indicating that the bits of any valid `T`
/// also correspond to a valid instance of this type. As such, it is safe to
/// construct an instance of this type simply be re-interpreting the bits of any
/// valid instance of `T`.
///
/// If `T: Sized` and `Self: Sized, then `T` is guaranteed to be at least as
/// large as `Self`. In other words, if `T: Sized` and `Self: Sized`, then
/// `Self: FitsIn<T>`.
///
/// If `T` is a DST, then the bits of any valid `T` with length
/// `mem::size_of::<Self>()` correspond to a valid instance of this type, but no
/// guarantees are made about sizes larger than `size_of::<Self>()`.
///
/// # Safety
///
/// Unsafe code may assume that types implementing this trait can be safely
/// constructed from the bits of a `T`. Implementing this trait for a type for
/// which this isn't actually safe may cause undefined behavior.
pub unsafe trait FromBits<T>
where
    T: ?Sized,
{
}

unsafe impl<T> FromBits<T> for [u8] {}

// NOTE on FitsIn and AlignedTo: Currently, these traits use constant evaluation
// to create a constant whose evaluation results in a divide-by-zero error if a
// particular boolean expression evaluates to false. Because this error is
// encountered during constant evaluation, it will only be triggered if the
// constant is actually accessed. Thus, both traits have a const_assert_xxx
// associated function that must be called in order to trigger the error.
//
// Eventually, Rust will add support for constant expressions in array lengths
// (https://github.com/rust-lang/rust/issues/43408). When this happens, we will
// be able to use that to trigger the divide-by-zero error during type checking,
// at which point:
// - We can remove the const_assert_xx functions
// - We will need to remove the blanket impl
// - We will probably want to add specific impls (e.g., T: FitsIn<T>,
//   u8: FitsIn<u16>, etc)

/// Types which are no larger than `T`.
///
/// If a type is `FitsIn<T>`, then `mem::size_of::<Self>() <=
/// mem::size_of::<T>()`.
///
/// Currently, unsafe code may *not* assume that `T: FitsIn<U>` guarantees that
/// `T` fits in `U`. It must call `T::const_assert_fits_in()`, which will cause
/// a compile-time error such as:
///
/// ```text
/// error[E0080]: constant evaluation error
///   --> src/main.rs:12:21
///    |
/// 12 |     const BAD: u8 = 1u8 / ((std::mem::size_of::<T>() >= std::mem::size_of::<Self>()) as
/// u8); |
/// ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ attempt to divide by
/// zero ```
///
/// if `T` does not fit in `U`.
pub unsafe trait FitsIn<T>
where
    T: Sized,
    Self: Sized,
{
    #[doc(hidden)]
    const BAD: u8 = 1u8 / ((mem::size_of::<T>() >= mem::size_of::<Self>()) as u8);

    fn const_assert_fits_in() {
        let _ = Self::BAD;
    }
}

// While any pair of types will type check, any (T, U) for which U is larger
// than T will fail during constant evaluation.
unsafe impl<T, U> FitsIn<T> for U {}

/// Types with alignment requirements at least as strict as those of `T`.
///
/// If a type is `AlignedTo<T>`, then any validly-aligned instance of it is
/// guaranteed to satisfy the alignment requirements of `T`.
///
/// Currently, unsafe code may *not* assume that `T: AlignedTo<U>` guarantees
/// that `T` satisfies `U`'s alignment requirements. It must call
/// `T::const_assert_aligned_to()`, which will cause a compile-time error such
/// as:
///
/// ```text
/// error[E0080]: constant evaluation error
///   --> src/main.rs:12:21
///    |
/// 12 |     const BAD: u8 = 1u8 / ((std::mem::align_of::<T>() <= std::mem::align_of::<Self>()) as
/// u8); |
/// ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ attempt to divide by
/// zero ```
///
/// if `T`'s alignment requirement is less strict than `U`'s.
pub unsafe trait AlignedTo<T>
where
    // TODO(joshlf): Remove this bound once there's a way to get the
    // align_of an unsized value
    Self: Sized,
{
    #[doc(hidden)]
    const BAD: u8 = 1u8 / ((mem::align_of::<T>() <= mem::align_of::<Self>()) as u8);

    fn const_assert_aligned_to() {
        let _ = Self::BAD;
    }
}

// While any pair of types will type check, any (T, U) for which U has less
// strict alignment requirements than T will fail during constant evaluation.
unsafe impl<T, U> AlignedTo<T> for U {}

/// Reinterpret the bits of one type as another type.
///
/// Unlike `std::mem::transmute`, `transmute` allows `T` and `U` to have
/// different sizes so long as `T` is larger than `U`. In that case, the return
/// value is constructed from the first `std::mem::size_of::<U>()` bytes of `x`.
/// Otherwise, `transmute` is identical to `std::mem::transmute`. In particular,
/// `x` is forgotten; it is not dropped.
pub unsafe fn transmute<T, U>(x: T) -> U
where
    U: FitsIn<T>,
{
    U::const_assert_fits_in();
    let ret = ptr::read(&x as *const T as *const U);
    mem::forget(x);
    ret
}

/// Safely reinterpret the bits of one type as another type.
///
/// `coerce` is like `transmute`, except that the `U: FromBits<T>` bound ensures
/// that the conversion is safe.
pub fn coerce<T, U>(x: T) -> U
where
    U: FromBits<T>,
{
    let ret = unsafe { ptr::read(&x as *const T as *const U) };
    mem::forget(x);
    ret
}

/// Safely coerce an immutable reference.
///
/// `coerce_ref` coerces an immutable reference to `T` into an immutable
/// reference to `U`, provided that any instance of `T` is a valid instance of
/// `U`, and that `T`'s alignment requirements are no less strict than `U`'s.
pub fn coerce_ref<T, U>(x: &T) -> &U
where
    U: FromBits<T>,
    T: AlignedTo<U>,
{
    T::const_assert_aligned_to();
    unsafe { &*(x as *const T as *const U) }
}

/// Safely coerce an immutable reference, checking size at runtime.
///
/// `coerce_ref_size_checked` coerces an immutable reference to `T` into an
/// immutable reference to `U`, provided that any instance of `T` is a valid
/// instance of `U`, and that `x` has the same size as `U`. If `x` has a
/// different size than `U`, `coerce_ref_size_checked` returns `None`.
pub fn coerce_ref_size_checked<T, U>(x: &T) -> Option<&U>
where
    T: ?Sized + AlignedTo<U>,
    U: FromBits<T>,
{
    if mem::size_of_val(x) != mem::size_of::<U>() {
        return None;
    }
    Some(unsafe { &*(x as *const T as *const U) })
}

/// Safely coerce an immutable reference, checking alignment at runtime.
///
/// `coerce_ref_align_checked` coerces an immutable reference to `T` into an
/// immutable reference to `U`, provided that any instance of `T` is a valid
/// instance of `U`, and that `x` satisfies `U`'s alignment requirements. If `x`
/// does not satisfy `U`'s alignment requirements, `coerce_ref_align_checked`
/// returns `None`.
pub fn coerce_ref_align_checked<T, U>(x: &T) -> Option<&U>
where
    U: FromBits<T>,
{
    if (x as *const T as usize) % mem::align_of::<U>() != 0 {
        return None;
    }
    Some(unsafe { &*(x as *const T as *const U) })
}

/// Safely coerce an immutable reference, checking size and alignment at runtime.
///
/// `coerce_ref_size_align_checked` coerces an immutable reference to `T` into
/// an immutable reference to `U`, provided that any instance of `T` is a valid
/// instance of `U`, that `x` has the same size as `U`, and that `x` satisfies
/// `U`'s alignment requirements. If `x` has a different size than `U` or does
/// not satisfy `U`'s alignment requirements, `coerce_ref_size_align_checked`
/// returns `None`.
pub fn coerce_ref_size_align_checked<T, U>(x: &T) -> Option<&U>
where
    T: ?Sized,
    U: FromBits<T>,
{
    if mem::size_of_val(x) != mem::size_of::<U>()
        && (x as *const _ as *const () as usize) % mem::align_of::<U>() != 0
    {
        return None;
    }
    Some(unsafe { &*(x as *const T as *const U) })
}

/// Safely coerce a mutable reference.
///
/// `coerce_mut` coerces a mutable reference to `T` into a mutable reference to
/// `U`, provided that any instance of `T` is a valid instance of `U`, any
/// instance of `U` is a valid instance of `T`, and that `T`'s alignment
/// requirements are no less strict than `U`'s.
pub fn coerce_mut<T, U>(x: &mut T) -> &mut U
where
    U: FromBits<T>,
    T: FromBits<U>,
    T: AlignedTo<U>,
{
    T::const_assert_aligned_to();
    unsafe { &mut *(x as *mut T as *mut U) }
}

/// Safely coerce a mutable reference, checking size at runtime.
///
/// `coerce_mut_size_checked` coerces a mutable reference to `T` into a mutable
/// reference to `U`, provided that any instance of `T` is a valid instance of
/// `U`, any instance of `U` is a valid instance of `T`, that `T`'s alignment
/// requirements are no less strict than `U`'s, and that `x` has the same size
/// as `U`. If `x` has a different size than `U`, `coerce_mut_size_checked`
/// returns `None`.
pub fn coerce_mut_size_checked<T, U>(x: &mut T) -> Option<&mut U>
where
    T: ?Sized + FromBits<U> + AlignedTo<U>,
    U: FromBits<T>,
{
    if mem::size_of_val(x) != mem::size_of::<U>() {
        return None;
    }
    Some(unsafe { &mut *(x as *mut T as *mut U) })
}

/// Safely coerce a mutable reference, checking alignment at runtime.
///
/// `coerce_mut_align_checked` coerces a mutable reference to `T` into a mutable
/// reference to `U`, provided that any instance of `T` is a valid instance of
/// `U`, any instance of `U` is a valid instance of `T`, and that `x` satisfies
/// `U`'s alignment requirements. If `x` does not satisfy `U`'s alignment
/// requirements, `coerce_mut_align_checked` returns `None`.
pub fn coerce_mut_align_checked<T, U>(x: &mut T) -> Option<&mut U>
where
    T: FromBits<U>,
    U: FromBits<T>,
{
    if (x as *const T as usize) % mem::align_of::<U>() != 0 {
        return None;
    }
    Some(unsafe { &mut *(x as *mut T as *mut U) })
}

/// Safely coerce a mutable reference, checking size and alignment at runtime.
///
/// `coerce_mut_size_align_checked` coerces a mutable reference to `T` into a
/// mutable reference to `U`, provided that any instance of `T` is a valid
/// instance of `U`, any instance of `U` is a valid instance of `T`, `x` has the
/// same size as `U`, and that `x` satisfies `U`'s alignment requirements. If
/// `x` has a different size than `U` or does not satisfy `U`'s alignment
/// requirements, `coerce_mut_size_align_checked` returns `None`.
pub fn coerce_mut_size_align_checked<T, U>(x: &T) -> Option<&U>
where
    T: ?Sized + FromBits<U>,
    U: FromBits<T>,
{
    if mem::size_of_val(x) != mem::size_of::<U>()
        && (x as *const _ as *const () as usize) % mem::align_of::<U>() != 0
    {
        return None;
    }
    Some(unsafe { &*(x as *const T as *const U) })
}

/// Coerce an immutable reference without checking size.
///
/// `coerce_ref_size_unchecked` coerces an immutable reference to `T` into an
/// immutable reference to `U`, provided that any properly-sized instance of `T`
/// is a valid instance of `U`. It is the caller's responsibility to ensure that
/// `x` is equal in size to `U`.
///
/// # Safety
///
/// If `x` is not equal in size to `U`, it may cause undefined behavior.
pub unsafe fn coerce_ref_size_unchecked<T, U>(x: &T) -> &U
where
    T: ?Sized + AlignedTo<U>,
    U: FromBits<T>,
{
    &*(x as *const T as *const U)
}

/// Coerce an immutable reference without checking alignment.
///
/// `coerce_ref_align_unchecked` coerces an immutable reference to `T` into an
/// immutable reference to `U`, provided that any instance of `T` is a valid
/// instance of `U`. It is the caller's responsibility to ensure that `x`
/// satisfies `U`'s alignment requirements.
///
/// # Safety
///
/// If `x` does not satisfy `U`'s alignment, it may result in undefined
/// behavior.
pub unsafe fn coerce_ref_align_unchecked<T, U>(x: &T) -> &U
where
    U: FromBits<T>,
{
    &*(x as *const T as *const U)
}

/// Coerce an immutable reference without checking size or alignment.
///
/// `coerce_ref_align_unchecked` coerces an immutable reference to `T` into an
/// immutable reference to `U`, provided that any properly-sized instance of `T`
/// is a valid instance of `U`. It is the caller's responsibility to ensure that
/// `x` is equal in size to `U` and that it satisfies `U`'s alignment
/// requirements.
///
/// # Safety
///
/// If `x` is not equal in size to `U` or does not satisfy `U`'s alignment, it
/// may result in undefined behavior.
pub unsafe fn coerce_ref_size_align_unchecked<T, U>(x: &T) -> &U
where
    T: ?Sized,
    U: FromBits<T>,
{
    &*(x as *const T as *const U)
}

/// Coerce a mutable reference without checking size.
///
/// `coerce_mut_size_unchecked` coerces a mutable reference to `T` into a
/// mutable reference to `U`, provided that any properly-sized instance of `T`
/// is a valid instance of `U` and that any instance of `U` is a valid instance
/// of `T`. It is the caller's responsibility to ensure that `x` is equal in
/// size to `U`.
///
/// # Safety
///
/// If `x` is not equal in size to `U`, it may cause undefined behavior.
pub unsafe fn coerce_mut_size_unchecked<T, U>(x: &mut T) -> &mut U
where
    T: ?Sized + AlignedTo<U> + FromBits<U>,
    U: FromBits<T>,
{
    &mut *(x as *mut T as *mut U)
}

/// Coerce a mutable reference without checking alignment.
///
/// `coerce_mut_align_unchecked` coerces a mutable reference to `T` into a
/// mutable reference to `U`, provided that any instance of `T` is a valid
/// instance of `U` and that any instance of `U` is a valid instance of `T`. It
/// is the caller's responsibility to ensure that `x` satisfies `U`'s alignment
/// requirements.
///
/// # Safety
///
/// If `x` does not satisfy `U`'s alignment, it may result in undefined
/// behavior.
pub unsafe fn coerce_mut_align_unchecked<T, U>(x: &mut T) -> &mut U
where
    T: FromBits<U>,
    U: FromBits<T>,
{
    &mut *(x as *mut T as *mut U)
}

/// Coerce a mutable reference without checking size or alignment.
///
/// `coerce_mut_size_align_unchecked` coerces a mutable reference to `T` into a
/// mutable reference to `U`, provided that any properly-sized instance of `T`
/// is a valid instance of `U` and that any instance of `U` is a valid instance
/// of `T`. It is the caller's responsibility to ensure that `x` is equal in
/// size to `U` and that it satisfies `U`'s alignment requirements.
///
/// # Safety
///
/// If `x` is not equal in size to `U` or does not satisfy `U`'s alignment, it
/// may results in undefined behavior.
pub unsafe fn coerce_mut_size_align_unchecked<T, U>(x: &mut T) -> &mut U
where
    T: ?Sized + FromBits<U>,
    U: FromBits<T>,
{
    &mut *(x as *mut T as *mut U)
}

/// A length- and alignment-checked reference to an object which can safely
/// be reinterpreted as another type.
///
/// `LayoutVerified` is an owned reference with the invaraint that the
/// referent's length and alignment are each greater than or equal to the length
/// and alignment of `U`. Using this invariant, it implements `Deref` and
/// `DerefMut` for `U`.
pub struct LayoutVerified<T, U>(T, PhantomData<U>);

impl<T, U> LayoutVerified<T, U>
where
    T: TrustedDeref,
{
    /// Construct a new `LayoutVerified`.
    ///
    /// `new` verifies that `x` is at least as large as `mem::size_of::<U>()`
    /// and that it satisfies `U`'s alignment requirements.
    pub fn new(x: T) -> Option<LayoutVerified<T, U>> {
        if mem::size_of_val(x.deref()) < mem::size_of::<U>()
            || (x.deref() as *const _ as *const () as usize) % mem::align_of::<U>() != 0
        {
            return None;
        }
        Some(LayoutVerified(x, PhantomData))
    }
}

impl<A, T, U> LayoutVerified<A, U>
where
    A: TrustedDeref<Target = [T]> + SplitAt,
{
    /// Construct a new `LayoutVerified` from the prefix of another type.
    ///
    /// `new_prefix` verifies that `x` is at least as large as
    /// `mem::size_of::<U>()` and that it satisfies `U`'s alignment
    /// requirements. It splits `x` at the smallest index such that the first
    /// half of the split is no smaller than `mem::size_of::<U>()`, uses the
    /// first half of the split to construct a `LayoutVerified`, and returns the
    /// second half of the split. If `mem::size_of::<U>()` is a multiple of
    /// `A`'s element size, then the first half's length is simply
    /// `mem::size_of::<U>()` divided by that element size.
    pub fn new_prefix(x: A) -> Option<(LayoutVerified<A, U>, A)> {
        if mem::size_of_val(x.deref()) < mem::size_of::<U>()
            || (x.deref() as *const _ as *const () as usize) % mem::align_of::<U>() != 0
        {
            return None;
        }
        Some(Self::new_prefix_helper(x))
    }
}

impl<T, U> LayoutVerified<T, U>
where
    T: TrustedDeref,
    T: AlignedTo<U>,
{
    /// Construct a new `LayoutVerified` with statically-guaranteed alignment.
    ///
    /// `new_aligned` verifies that `x` is at least as large as
    /// `mem::size_of::<U>()`.
    ///
    /// `T::Target`'s alignment guarantees must be at least as strict as `U`'s
    /// so that a reference to `T::Target` can be converted to a reference to
    /// `U` without violating `U`'s alignment requirements.
    pub fn new_aligned(x: T) -> Option<LayoutVerified<T, U>> {
        T::const_assert_aligned_to();
        if mem::size_of_val(x.deref()) < mem::size_of::<U>() {
            return None;
        }
        Some(LayoutVerified(x, PhantomData))
    }
}

impl<A, T, U> LayoutVerified<A, U>
where
    A: TrustedDeref<Target = [T]> + SplitAt,
    A: AlignedTo<U>,
{
    /// Construct a new `LayoutVerified` with statically-guaranteed alignment
    /// from the prefix of another type.
    ///
    /// `new_aligned_prefix` verifies that `x` is at least as large as
    /// `mem::size_of::<U>()` and that it satisfies `U`'s alignment
    /// requirements. It splits `x` at the smallest index such that the first
    /// half of the split is no smaller than `mem::size_of::<U>()`, uses the
    /// first half of the split to construct a `LayoutVerified`, and returns the
    /// second half of the split. If `mem::size_of::<U>()` is a multiple of
    /// `A`'s element size, then the first half's length is simply
    /// `mem::size_of::<U>()` divided by that element size.
    ///
    /// `A::Target`'s alignment guarantees must be at least as strict as `U`'s
    /// so that a reference to `A::Target` can be converted to a reference to
    /// `U` without violating `U`'s alignment requirements.
    pub fn new_aligned_prefix(x: A) -> Option<(LayoutVerified<A, U>, A)> {
        A::const_assert_aligned_to();
        if mem::size_of_val(x.deref()) < mem::size_of::<U>() {
            return None;
        }
        Some(Self::new_prefix_helper(x))
    }
}

impl<T, U> LayoutVerified<T, U>
where
    T: TrustedDeref,
    T::Target: Sized,
    U: FitsIn<T::Target>,
{
    /// Construct a new `LayoutVerified` with statically-guaranteed size.
    ///
    /// `new_aligned` verifies that `x` satisfies `U`'s alignment requirements.
    ///
    /// `T::Target` must be at least as large as `U`.
    pub fn new_sized(x: T) -> Option<LayoutVerified<T, U>> {
        U::const_assert_fits_in();
        if (x.deref() as *const T::Target as usize) % mem::align_of::<U>() != 0 {
            return None;
        }
        Some(LayoutVerified(x, PhantomData))
    }
}

impl<T, U> LayoutVerified<T, U>
where
    T: TrustedDeref,
    T::Target: Sized,
    U: FitsIn<T::Target>,
    T: AlignedTo<U>,
{
    /// Construct a new `LayoutVerified` with statically-guaranteed size and
    /// alignment.
    ///
    /// `T::Target` must be at least as large as `U`. `T::Target`'s alignment
    /// guarantees must be at least as strict as `U`'s so that a reference to
    /// `T::Target` can be converted to a reference to `U` without violating
    /// `U`'s alignment requirements.
    pub fn new_sized_aligned(x: T) -> LayoutVerified<T, U> {
        U::const_assert_fits_in();
        T::const_assert_aligned_to();
        LayoutVerified(x, PhantomData)
    }
}

impl<A, T, U> LayoutVerified<A, U>
where
    A: TrustedDeref<Target = [T]> + SplitAt,
{
    fn new_prefix_helper(x: A) -> (LayoutVerified<A, U>, A) {
        let idx = if mem::size_of_val(x.deref()) % mem::size_of::<T>() == 0 {
            mem::size_of_val(x.deref()) / mem::size_of::<T>()
        } else {
            (mem::size_of_val(x.deref()) / mem::size_of::<T>()) + 1
        };
        let (x, rest) = x.split_at(idx);
        (LayoutVerified(x, PhantomData), rest)
    }
}

impl<T, U> LayoutVerified<T, U> {
    /// Get the underlying `T`.
    ///
    /// `get_t` returns a reference to the `T` backing this `LayoutVerified`.
    pub fn get_t(&self) -> &T {
        &self.0
    }
}

impl<T, U> Deref for LayoutVerified<T, U>
where
    T: TrustedDeref,
    U: FromBits<T::Target>,
{
    type Target = U;
    fn deref(&self) -> &U {
        unsafe { &*((&self.0) as *const _ as *const U) }
    }
}

impl<T, U> DerefMut for LayoutVerified<T, U>
where
    T: TrustedDerefMut,
    U: FromBits<T::Target>,
    T::Target: FromBits<U>,
{
    fn deref_mut(&mut self) -> &mut U {
        unsafe { &mut *((&mut self.0) as *mut _ as *mut U) }
    }
}

/// Like `Deref`, but guaranteed to always return the same length.
///
/// `TrustedDeref` is like `Deref`, but multiple calls to `deref` on the same
/// object are always guaranteed to return references to objects of the same
/// size.
///
/// # Safety
///
/// Unsafe code may rely on the size-consistency property, so violating that
/// property may cause undefined behavior.
pub unsafe trait TrustedDeref: Deref {}

/// Like `DerefMut`, but guaranteed to always return the same length.
///
/// `TrustedDerefMut` is like `DerefMut`, but multiple calls to `deref` on the
/// same object are always guaranteed to return references to objects of the
/// same size.
///
/// # Safety
///
/// Unsafe code may rely on the size-consistency property, so violating that
/// property may cause undefined behavior.
pub unsafe trait TrustedDerefMut: TrustedDeref + DerefMut {}

// unsafe impl<'a> TrustedDeref for &'a [u8] {}
// unsafe impl<'a> TrustedDerefMut for &'a mut [u8] {}

/// Types which can be split at an index.
///
/// Types which implement `SplitAt` must guarantee that splits work as expected
/// - if an object has a length, `len`, and then is split at index `idx`, the
/// first return value will have length `idx` and the second will have length
/// `len - idx`.
///
/// # Safety
///
/// Unsafe code may rely on `SplitAt` types to behave as documented, so
/// violating this documentation may cause undefined behavior.
pub unsafe trait SplitAt: Sized {
    fn split_at(self, mid: usize) -> (Self, Self);
}

unsafe impl<'a> SplitAt for &'a [u8] {
    fn split_at(self, mid: usize) -> (Self, Self) {
        <[u8]>::split_at(self, mid)
    }
}
