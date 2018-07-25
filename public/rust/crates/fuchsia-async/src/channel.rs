// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::io;
use std::fmt;

use futures::{Future, Poll, task, try_ready};
use fuchsia_zircon::{self as zx, AsHandleRef, MessageBuf};

use crate::RWHandle;

/// An I/O object representing a `Channel`.
pub struct Channel(RWHandle<zx::Channel>);

impl AsRef<zx::Channel> for Channel {
    fn as_ref(&self) -> &zx::Channel {
        self.0.get_ref()
    }
}

impl AsHandleRef for Channel {
    fn as_handle_ref(&self) -> zx::HandleRef {
        self.0.get_ref().as_handle_ref()
    }
}

impl From<Channel> for zx::Channel {
    fn from(channel: Channel) -> zx::Channel {
        channel.0.into_inner()
    }
}

impl Channel {
    /// Creates a new `Channel` from a previously-created `zx::Channel`.
    pub fn from_channel(channel: zx::Channel) -> io::Result<Channel> {
        Ok(Channel(RWHandle::new(channel)?))
    }

    /// Tests to see if the channel received a OBJECT_PEER_CLOSED signal
    pub fn is_closed(&self) -> bool {
        self.0.is_closed()
    }

    /// Test whether this socket is ready to be read or not.
    ///
    /// If the socket is *not* readable then the current task is scheduled to
    /// get a notification when the socket does become readable. That is, this
    /// is only suitable for calling in a `Future::poll` method and will
    /// automatically handle ensuring a retry once the socket is readable again.
    fn poll_read(&self, cx: &mut task::Context) -> Poll<Result<(), zx::Status>> {
        self.0.poll_read(cx)
    }

    /// Receives a message on the channel and registers this `Channel` as
    /// needing a read on receiving a `zx::Status::SHOULD_WAIT`.
    pub fn recv_from(&self, buf: &mut MessageBuf, cx: &mut task::Context)
        -> Poll<Result<(), zx::Status>>
    {
        try_ready!(self.poll_read(cx));

        let res = self.0.get_ref().read(buf);
        if res == Err(zx::Status::SHOULD_WAIT) {
            return match self.0.need_read(cx) {
                Ok(()) => Poll::Pending,
                Err(e) => Poll::Ready(Err(e)),
            };
        }
        Poll::Ready(res)
    }

    /// Creates a future that reads a message into the buffer provided.
    ///
    /// The returned future will return after a message has been received on
    /// this socket or an error has occured.
    pub fn recv_msg<'a>(&'a self, buf: &'a mut MessageBuf)
        -> impl Future<Output = Result<(), zx::Status>> + 'a
    {
        futures::future::poll_fn(move |cx| self.recv_from(buf, cx))
    }

    /// Returns a `Future` that continuously reads messages from the channel
    /// and calls the callback with them, re-using the message buffer. The
    /// callback returns a future that serializes the server loop so it won't
    /// read the next message until the future returns and gives it a
    /// channel and buffer back.
    pub async fn chain_server<F, Fut>(self, mut callback: F) -> Result<(Channel, MessageBuf), zx::Status>
    where
        F: FnMut(Channel, MessageBuf) -> Fut,
        Fut: Future<Output = Result<(Channel, MessageBuf), zx::Status>>,
    {
        let mut chan = self;
        let mut buf = MessageBuf::new();
        loop {
            await!(chan.recv_msg(&mut buf))?;
            let res = await!(callback(chan, buf))?;
            chan = res.0;
            buf = res.1;
            buf.clear();
        }
    }

    /// Writes a message into the channel.
    pub fn write(&self,
                 bytes: &[u8],
                 handles: &mut Vec<zx::Handle>,
                ) -> Result<(), zx::Status>
    {
        self.0.get_ref().write(bytes, handles)
    }
}

impl fmt::Debug for Channel {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.get_ref().fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use crate::{Executor, Timer, TimeoutExt};
    use fuchsia_zircon::{
        self as zx,
        MessageBuf,
        prelude::*,
    };
    use futures::prelude::*;
    use super::*;

    #[test]
    fn can_receive() {
        let mut exec = Executor::new().unwrap();
        let bytes = &[0,1,2,3];

        let (tx, rx) = zx::Channel::create().unwrap();
        let f_rx = Channel::from_channel(rx).unwrap();

        let mut buffer = MessageBuf::new();
        let receiver = async {
            await!(f_rx.recv_msg(&mut buffer));
            assert_eq!(bytes, buffer.bytes());
        };

        // add a timeout to receiver so if test is broken it doesn't take forever
        let receiver = receiver.on_timeout(
                        1000.millis().after_now(),
                        || panic!("timeout"));

        let sender = async {
            await!(Timer::new(100.millis().after_now()));
            let mut handles = Vec::new();
            tx.write(bytes, &mut handles).unwrap();
        };

        let done = receiver.join(sender);
        exec.run_singlethreaded(done);
    }

    #[test]
    fn chain_server() {
        let mut exec = Executor::new().unwrap();

        let (tx, rx) = zx::Channel::create().unwrap();
        let f_rx = Channel::from_channel(rx).unwrap();

        let mut count = 0;
        let receiver = f_rx.chain_server(async move |chan, buf| {
            println!("got bytes: {}: {:?}", count, buf.bytes());
            assert_eq!(1, buf.bytes().len());
            assert_eq!(count, buf.bytes()[0]);
            count += 1;
            await!(Timer::new(100.millis().after_now()));
            Ok((chan, buf))
        }).map(|res| { res.unwrap(); });

        // add a timeout to receiver to stop the server eventually
        let receiver = receiver.on_timeout(400.millis().after_now(), || ());

        let sender = async {
            await!(Timer::new(100.millis().after_now()));
            let mut handles = Vec::new();
            tx.write(&[0], &mut handles).unwrap();
            tx.write(&[1], &mut handles).unwrap();
            tx.write(&[2], &mut handles).unwrap();
        };

        let done = receiver.join(sender);
        exec.run_singlethreaded(done);
    }

    #[test]
    fn chain_server_pre_write() {
        let mut exec = Executor::new().unwrap();

        let (tx, rx) = zx::Channel::create().unwrap();
        tx.write(b"txidhelloworld", &mut vec![]).unwrap();
        drop(tx);

        let f_rx = Channel::from_channel(rx).unwrap();

        let mut received = None;
        {
            let receiver = f_rx.chain_server(|chan, buf| {
                received = Some(buf.bytes().to_owned());
                futures::future::ready(Ok((chan, buf)))
            }).map(|res| { res.unwrap(); });

            exec.run_singlethreaded(receiver);
        }
        assert_eq!(Some(b"txidhelloworld".to_vec()), received);
    }
}
