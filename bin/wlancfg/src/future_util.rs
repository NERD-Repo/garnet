// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use async::Interval;
use futures::prelude::*;
use zx;

pub async fn retry_until<T, E, FUNC, FUT>(retry_interval: zx::Duration, mut f: FUNC)
    -> Result<T, E>
    where FUNC: FnMut() -> FUT,
          FUT: Future<Output = Result<Option<T>, E>>
{
    if let Some(res) = await!(f())? {
        return Ok(res);
    }

    let mut interval = Interval::new(retry_interval);
    while let Some(()) = await!(interval.next()) {
        if let Some(res) = await!(f())? {
            return Ok(res);
        }
    }
    unreachable!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use async;
    use zx::prelude::*;

    #[test]
    fn first_try() {
        let mut exec = async::Executor::new().expect("failed to create an executor");
        let mut fut = retry_until::<_, Never, _, _>(5.seconds(), || future::ok(Some(123)));
        assert_eq!(Ok(Async::Ready(123)), exec.run_until_stalled(&mut fut));
    }

    #[test]
    fn third_try() {
        let mut exec = async::Executor::new().expect("failed to create an executor");
        let mut countdown = 3;
        let start = 0.seconds().after_now();
        let mut fut = retry_until::<_, Never, _, _>(
            5.seconds(),
            move || {
                countdown -= 1;
                if countdown == 0 {
                    future::ok(Some(123))
                } else {
                    future::ok(None)
                }
            });
        assert_eq!(Ok(Async::Pending), exec.run_until_stalled(&mut fut));
        let first_timeout = exec.wake_next_timer().expect("expected a pending timer");
        assert!(first_timeout >= start + 5.seconds());
        assert_eq!(Ok(Async::Pending), exec.run_until_stalled(&mut fut));
        let second_timeout = exec.wake_next_timer().expect("expected a pending timer");
        assert_eq!(first_timeout + 5.seconds(), second_timeout);
        assert_eq!(Ok(Async::Ready(123)), exec.run_until_stalled(&mut fut));
    }
}
