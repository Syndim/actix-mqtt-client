use std::boxed::Box;
use std::io::Error;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures::{AsyncRead as FutureAsyncRead, AsyncWrite as FutureAsyncWrite};
use tokio::io::{AsyncRead as TokioAsyncRead, AsyncWrite as TokioAsyncWrite};

pub(crate) struct Stream<S> {
    s: Pin<Box<S>>,
}

impl<S> Stream<S>
where
    S: Unpin,
{
    #[allow(dead_code)]
    pub fn new(s: S) -> Self {
        Stream { s: Box::pin(s) }
    }
}

impl<S: TokioAsyncRead> FutureAsyncRead for Stream<S> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &mut [u8],
    ) -> Poll<Result<usize, Error>> {
        Pin::into_inner(self).s.as_mut().poll_read(cx, buf)
    }
}

impl<S: TokioAsyncWrite + Unpin> FutureAsyncWrite for Stream<S> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &[u8],
    ) -> Poll<Result<usize, Error>> {
        Pin::into_inner(self).s.as_mut().poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), Error>> {
        Pin::into_inner(self).s.as_mut().poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), Error>> {
        Pin::into_inner(self).s.as_mut().poll_shutdown(cx)
    }
}
