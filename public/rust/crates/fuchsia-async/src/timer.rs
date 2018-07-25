// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Support for creating futures that represent timers.
//!
//! This module contains the `Timer` type which is a future that will resolve
//! at a particular point in the future.

use std::marker::Unpin;
use std::mem::PinMut;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use futures::{Future, FutureExt, Stream, Poll, unsafe_unpinned, unsafe_pinned};
use futures::task::{self, AtomicWaker};
use fuchsia_zircon as zx;

use crate::executor::EHandle;

/// A trait which allows futures to be easily wrapped in a timeout.
pub trait TimeoutExt: Future + Sized {
    /// Wraps the future in a timeout, calling `on_timeout` to produce a result
    /// when the timeout occurs.
    fn on_timeout<OT>(self, time: zx::Time, on_timeout: OT)
        -> OnTimeout<Self, OT>
    where
        OT: FnOnce() -> Self::Output
    {
        OnTimeout {
            timer: Timer::new(time),
            future: self,
            on_timeout: Some(on_timeout),
        }
    }
}

impl<F: Future + Sized> TimeoutExt for F {}

/// A wrapper for a future which will complete with a provided closure when a timeout occurs.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct OnTimeout<F, OT> {
    timer: Timer,
    future: F,
    on_timeout: Option<OT>,
}

impl<F: Unpin, OT> Unpin for OnTimeout<F, OT> {}

impl<F, OT> OnTimeout<F, OT> {
    unsafe_unpinned!(timer: Timer);
    unsafe_pinned!(future: F);
    unsafe_unpinned!(on_timeout: Option<OT>);
}

impl<F: Future, OT> Future for OnTimeout<F, OT>
    where OT: FnOnce() -> F::Output
{
    type Output = F::Output;
    fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
        if let Poll::Ready(item) = self.future().poll(cx) {
            return Poll::Ready(item);
        }
        if let Poll::Ready(()) = self.timer().poll_unpin(cx) {
            let ot = OnTimeout::on_timeout(&mut self).take()
                .expect("polled withtimeout after completion");
            let item = (ot)();
            return Poll::Ready(item);
        }
        Poll::Pending
    }
}

/// An asynchronous timer.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Timer {
    waker_and_bool: Arc<(AtomicWaker, AtomicBool)>,
}

impl Timer {
    /// Create a new timer scheduled to fire at `time`.
    pub fn new(time: zx::Time) -> Self {
        let waker_and_bool = Arc::new((AtomicWaker::new(), AtomicBool::new(false)));
        EHandle::local().register_timer(time, &waker_and_bool);
        Timer { waker_and_bool }
    }

    /// Reset the `Timer` to a fire at a new time.
    /// The `Timer` must have already fired since last being reset.
    pub fn reset(&mut self, time: zx::Time) {
        assert!(self.did_fire(Ordering::Relaxed));
        self.waker_and_bool.1.store(false, Ordering::SeqCst);
        EHandle::local().register_timer(time, &self.waker_and_bool)
    }

    fn did_fire(&self, ordering: Ordering) -> bool {
        self.waker_and_bool.1.load(ordering)
    }

    fn register_task(&self, cx: &mut task::Context) {
        self.waker_and_bool.0.register(cx.waker());
    }
}

impl Future for Timer {
    type Output = ();
    fn poll(self: PinMut<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
        if self.did_fire(Ordering::Relaxed) {
            return Poll::Ready(());
        }
        self.register_task(cx);
        if self.did_fire(Ordering::SeqCst) {
            return Poll::Ready(());
        }
        Poll::Pending
    }
}

/// An asynchronous interval timer.
/// This is a stream of events resolving at a rate of once-per interval.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Interval {
    timer: Timer,
    next: zx::Time,
    duration: zx::Duration,
}

impl Interval {
    /// Create a new `Interval` which yields every `duration`.
    pub fn new(duration: zx::Duration) -> Self {
        let next = duration.after_now();
        Interval {
            timer: Timer::new(next),
            next,
            duration,
        }
    }
}

impl Stream for Interval {
    type Item = ();
    fn poll_next(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Option<Self::Item>> {
        match self.timer.poll_unpin(cx) {
            Poll::Ready(()) => {
                self.next += self.duration;
                let next = self.next;
                self.timer.reset(next);
                Poll::Ready(Some(()))
            }
            Poll::Pending => {
                Poll::Pending
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{Executor, Timer};
    use futures::{pin_mut, select};
    use futures::prelude::*;
    use fuchsia_zircon::prelude::*;

    #[test]
    fn shorter_fires_first() {
        let mut exec = Executor::new().unwrap();
        let shorter = Timer::new(100.millis().after_now());
        let longer = Timer::new(1.second().after_now());
        let fut = async {
            pin_mut!(shorter, longer);
            select! {
                shorter => {},
                longer => panic!("wrong timer fired"),
            }
        };
        exec.run_singlethreaded(fut);
    }

    #[test]
    fn shorter_fires_first_multithreaded() {
        let mut exec = Executor::new().unwrap();
        let shorter = Timer::new(100.millis().after_now());
        let longer = Timer::new(1.second().after_now());
        let fut = async {
            pin_mut!(shorter, longer);
            select! {
                shorter => {},
                longer => panic!("wrong timer fired"),
            }
        };
        exec.run(fut, 4);
    }

    #[test]
    fn fires_after_timeout() {
        let mut exec = Executor::new().unwrap();
        let deadline = 5.seconds().after_now();
        let future = Timer::new(deadline);
        pin_mut!(future);
        assert_eq!(Poll::Pending, exec.run_until_stalled(future.reborrow()));
        assert_eq!(Some(deadline), exec.wake_next_timer());
        assert_eq!(Poll::Ready(()), exec.run_until_stalled(future.reborrow()));
    }

    #[test]
    fn interval() {
        let mut exec = Executor::new().unwrap();
        let start = 0.seconds().after_now();

        let counter = Arc::new(::std::sync::atomic::AtomicUsize::new(0));
        let future = async {
            let counter = counter.clone();
            let mut interval = Interval::new(5.seconds());
            while let Some(()) = await!(interval.next()) {
                counter.fetch_add(1, Ordering::SeqCst);
            }
        };
        pin_mut!(future);

        // Poll for the first time before the timer runs
        assert_eq!(Poll::Pending, exec.run_until_stalled(future.reborrow()));
        assert_eq!(0, counter.load(Ordering::SeqCst));

        // Pretend to wait until the next timer
        let first_deadline = exec.wake_next_timer().expect("Expected a pending timeout (1)");
        assert!(first_deadline >= start + 5.seconds());
        assert_eq!(Poll::Pending, exec.run_until_stalled(future.reborrow()));
        assert_eq!(1, counter.load(Ordering::SeqCst));

        // Polling again before the timer runs shouldn't produce another item from the stream
        assert_eq!(Poll::Pending, exec.run_until_stalled(future.reborrow()));
        assert_eq!(1, counter.load(Ordering::SeqCst));

        // "Wait" until the next timeout and poll again: expect another item from the stream
        let second_deadline = exec.wake_next_timer().expect("Expected a pending timeout (2)");
        assert_eq!(Poll::Pending, exec.run_until_stalled(future.reborrow()));
        assert_eq!(2, counter.load(Ordering::SeqCst));

        assert_eq!(second_deadline, first_deadline + 5.seconds());
    }

}
