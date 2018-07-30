// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! A futures-rs executor design specifically for Fuchsia OS.
#![feature(
    arbitrary_self_types,
    async_await,
    await_macro,
    futures_api,
    pin
)]
#![deny(warnings)]
#![deny(missing_docs)]

// Set the system allocator for anything using this crate
extern crate fuchsia_system_alloc;

/// A future which can be used by multiple threads at once.
pub mod atomic_future;

/// Re-export the futures crate for use from macros
#[doc(hidden)]
pub mod futures {
    pub use futures::*;
}

mod channel;
pub use self::channel::Channel;
mod on_signals;
pub use self::on_signals::OnSignals;
mod rwhandle;
pub use self::rwhandle::RWHandle;
mod socket;
pub use self::socket::Socket;
mod timer;
pub use self::timer::{Interval, OnTimeout, TimeoutExt, Timer};
mod executor;
pub use self::executor::{spawn, spawn_local, EHandle, Executor};
mod fifo;
pub use self::fifo::{Fifo, FifoEntry, FifoReadable, FifoWritable, ReadEntry, WriteEntry};
pub mod net;

// Safety: the resulting type cannot be given an `Unpin` or `Drop` implementation
#[macro_export]
macro_rules! unsafe_many_futures {
    ($future:ident, [$first:ident, $($subfuture:ident $(,)*)*]) => {
        pub enum $future<$first, $($subfuture,)*> {
            $first($first),
            $(
                $subfuture($subfuture),
            )*
        }

        impl<$first, $($subfuture,)*> $crate::futures::Future for $future<$first, $($subfuture,)*>
        where
            $first: $crate::futures::Future,
            $(
                $subfuture: $crate::futures::Future<Output = $first::Output>,
            )*
        {
            type Output = $first::Output;
            fn poll(self: PinMut<Self>, cx: &mut $crate::futures::task::Context)
                -> $crate::futures::Poll<Self::Output>
            {
                // Safety: direct projection of fields is allowed provided that the caller
                // upholds the required invariants (no `Unpin` or `Drop`)
                unsafe {
                    match PinMut::get_mut_unchecked(self) {
                        $future::$first(x) =>
                            $crate::futures::Future::poll(PinMut::new_unchecked(x), cx),
                        $(
                            $future::$subfuture(x) =>
                                $crate::futures::Future::poll(PinMut::new_unchecked(x), cx),
                        )*
                    }
                }
            }
        }
    }
}
