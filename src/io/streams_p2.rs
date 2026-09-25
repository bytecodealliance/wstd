use super::{AsyncPollable, AsyncRead, AsyncWrite};
use crate::runtime::WaitFor;
use std::future::{Future, poll_fn};
use std::pin::Pin;
use std::sync::{Mutex, OnceLock};
use std::task::{Context, Poll};
use wasip2::io::streams::{InputStream, OutputStream, StreamError};

/// A wrapper for WASI's `InputStream` resource that provides implementations of `AsyncRead` and
/// `AsyncPollable`.
#[derive(Debug)]
pub struct AsyncInputStream {
    wait_for: Mutex<Option<Pin<Box<WaitFor>>>>,
    // Lazily initialized pollable, used for lifetime of stream to check readiness.
    // Field ordering matters: this child must be dropped before stream
    subscription: OnceLock<AsyncPollable>,
    stream: InputStream,
}

impl AsyncInputStream {
    /// Construct an `AsyncInputStream` from a WASI `InputStream` resource.
    pub fn new(stream: InputStream) -> Self {
        Self {
            wait_for: Mutex::new(None),
            subscription: OnceLock::new(),
            stream,
        }
    }
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        // Lazily initialize the AsyncPollable
        let subscription = self
            .subscription
            .get_or_init(|| AsyncPollable::new(self.stream.subscribe()));
        // Lazily initialize the WaitFor. Clear it after it becomes ready.
        let mut wait_for_slot = self.wait_for.lock().unwrap();
        let wait_for = wait_for_slot.get_or_insert_with(|| Box::pin(subscription.wait_for()));
        match wait_for.as_mut().poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(()) => {
                let _ = wait_for_slot.take();
                Poll::Ready(())
            }
        }
    }
    /// Await for read readiness.
    async fn ready(&mut self) {
        poll_fn(|cx| self.poll_ready(cx)).await
    }
    /// Move the entire contents of an input stream directly into an output
    /// stream, until the input stream has closed. This operation is optimized
    /// to avoid copying stream contents into and out of memory.
    pub async fn copy_to(&mut self, writer: &mut AsyncOutputStream) -> std::io::Result<u64> {
        let mut written = 0;
        loop {
            self.ready().await;
            writer.ready().await;
            match writer.stream.splice(&self.stream, u64::MAX) {
                Ok(n) => written += n,
                Err(StreamError::Closed) => break Ok(written),
                Err(StreamError::LastOperationFailed(err)) => {
                    break Err(std::io::Error::other(err.to_debug_string()));
                }
            }
        }
    }

    /// Use this `AsyncInputStream` as a `futures_lite::stream::Stream` with
    /// items of `Result<Vec<u8>, std::io::Error>`. The returned byte vectors
    /// will be at most 8k. If you want to control chunk size, use
    /// `Self::into_stream_of`.
    pub fn into_stream(self) -> AsyncInputChunkStream {
        AsyncInputChunkStream {
            stream: self,
            chunk_size: 8 * 1024,
        }
    }

    /// Use this `AsyncInputStream` as a `futures_lite::stream::Stream` with
    /// items of `Result<Vec<u8>, std::io::Error>`. The returned byte vectors
    /// will be at most the `chunk_size` argument specified.
    ///
    /// # Panics
    ///
    /// Panics if `chunk_size` is zero.
    pub fn into_stream_of(self, chunk_size: usize) -> AsyncInputChunkStream {
        assert!(chunk_size > 0, "chunk size must be non-zero");
        AsyncInputChunkStream {
            stream: self,
            chunk_size,
        }
    }

    /// Use this `AsyncInputStream` as a `futures_lite::stream::Stream` with
    /// items of `Result<u8, std::io::Error>`.
    pub fn into_bytestream(self) -> AsyncInputByteStream {
        AsyncInputByteStream {
            stream: self.into_stream(),
            buffer: std::io::Read::bytes(std::io::Cursor::new(Vec::new())),
        }
    }
}

impl AsyncRead for AsyncInputStream {
    async fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let read = loop {
            self.ready().await;
            // Ideally, the ABI would be able to read directly into buf.
            // However, with the default generated bindings, it returns a
            // newly allocated vec, which we need to copy into buf.
            match self.stream.read(buf.len() as u64) {
                // A read of 0 bytes from WASI's `read` doesn't mean
                // end-of-stream as it does in Rust. However, `self.ready()`
                // cannot guarantee that at least one byte is ready for
                // reading, so in this case we try again.
                Ok(r) if r.is_empty() => continue,
                Ok(r) => break r,
                // 0 bytes from Rust's `read` means end-of-stream.
                Err(StreamError::Closed) => return Ok(0),
                Err(StreamError::LastOperationFailed(err)) => {
                    return Err(std::io::Error::other(err.to_debug_string()));
                }
            }
        };
        let len = read.len();
        buf[0..len].copy_from_slice(&read);
        Ok(len)
    }

    #[inline]
    fn as_async_input_stream(&mut self) -> Option<&mut AsyncInputStream> {
        Some(self)
    }
}

/// Wrapper of `AsyncInputStream` that impls `futures_lite::stream::Stream`
/// with an item of `Result<Vec<u8>, std::io::Error>`
pub struct AsyncInputChunkStream {
    stream: AsyncInputStream,
    chunk_size: usize,
}

impl AsyncInputChunkStream {
    /// Extract the `AsyncInputStream` which backs this stream.
    pub fn into_inner(self) -> AsyncInputStream {
        self.stream
    }
}

impl futures_lite::stream::Stream for AsyncInputChunkStream {
    type Item = Result<Vec<u8>, std::io::Error>;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        match this.stream.poll_ready(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(()) => match this.stream.stream.read(this.chunk_size as u64) {
                Ok(r) if r.is_empty() => Poll::Pending,
                Ok(r) => Poll::Ready(Some(Ok(r))),
                Err(StreamError::LastOperationFailed(err)) => {
                    Poll::Ready(Some(Err(std::io::Error::other(err.to_debug_string()))))
                }
                Err(StreamError::Closed) => Poll::Ready(None),
            },
        }
    }
}

pin_project_lite::pin_project! {
    /// Wrapper of `AsyncInputStream` that impls
    /// `futures_lite::stream::Stream` with item `Result<u8, std::io::Error>`.
    pub struct AsyncInputByteStream {
        #[pin]
        stream: AsyncInputChunkStream,
        buffer: std::io::Bytes<std::io::Cursor<Vec<u8>>>,
    }
}

impl AsyncInputByteStream {
    /// Extract the `AsyncInputStream` which backs this stream, and any bytes
    /// read from the `AsyncInputStream` which have not yet been yielded by
    /// the byte stream.
    pub fn into_inner(self) -> (AsyncInputStream, Vec<u8>) {
        (
            self.stream.into_inner(),
            self.buffer
                .collect::<Result<Vec<u8>, std::io::Error>>()
                .expect("read of Cursor<Vec<u8>> is infallible"),
        )
    }
}

impl futures_lite::stream::Stream for AsyncInputByteStream {
    type Item = Result<u8, std::io::Error>;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        match this.buffer.next() {
            Some(byte) => Poll::Ready(Some(Ok(byte.expect("cursor on Vec<u8> is infallible")))),
            None => match futures_lite::stream::Stream::poll_next(this.stream, cx) {
                Poll::Ready(Some(Ok(bytes))) => {
                    let mut bytes = std::io::Read::bytes(std::io::Cursor::new(bytes));
                    match bytes.next() {
                        Some(Ok(byte)) => {
                            *this.buffer = bytes;
                            Poll::Ready(Some(Ok(byte)))
                        }
                        Some(Err(err)) => Poll::Ready(Some(Err(err))),
                        None => Poll::Ready(None),
                    }
                }
                Poll::Ready(Some(Err(err))) => Poll::Ready(Some(Err(err))),
                Poll::Ready(None) => Poll::Ready(None),
                Poll::Pending => Poll::Pending,
            },
        }
    }
}

/// A wrapper for WASI's `output-stream` resource that provides implementations of `AsyncWrite` and
/// `AsyncPollable`.
#[derive(Debug)]
pub struct AsyncOutputStream {
    // Lazily initialized pollable, used for lifetime of stream to check readiness.
    // Field ordering matters: this child must be dropped before stream
    subscription: OnceLock<AsyncPollable>,
    stream: OutputStream,
}

impl AsyncOutputStream {
    /// Construct an `AsyncOutputStream` from a WASI `OutputStream` resource.
    pub fn new(stream: OutputStream) -> Self {
        Self {
            subscription: OnceLock::new(),
            stream,
        }
    }
    /// Await write readiness.
    async fn ready(&mut self) {
        // Lazily initialize the AsyncPollable
        let subscription = self
            .subscription
            .get_or_init(|| AsyncPollable::new(self.stream.subscribe()));
        // Wait on readiness
        subscription.wait_for().await;
    }
}

impl AsyncWrite for AsyncOutputStream {
    // Required methods
    async fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        // Loops at most twice.
        loop {
            match self.stream.check_write() {
                Ok(0) => {
                    self.ready().await;
                    // Next loop guaranteed to have nonzero check_write, or error.
                    continue;
                }
                Ok(some) => {
                    let writable = some.try_into().unwrap_or(usize::MAX).min(buf.len());
                    match self.stream.write(&buf[0..writable]) {
                        Ok(()) => return Ok(writable),
                        Err(StreamError::Closed) => {
                            return Err(std::io::Error::from(std::io::ErrorKind::ConnectionReset));
                        }
                        Err(StreamError::LastOperationFailed(err)) => {
                            return Err(std::io::Error::other(err.to_debug_string()));
                        }
                    }
                }
                Err(StreamError::Closed) => {
                    return Err(std::io::Error::from(std::io::ErrorKind::ConnectionReset));
                }
                Err(StreamError::LastOperationFailed(err)) => {
                    return Err(std::io::Error::other(err.to_debug_string()));
                }
            }
        }
    }
    async fn flush(&mut self) -> std::io::Result<()> {
        match self.stream.flush() {
            Ok(()) => {
                self.ready().await;
                Ok(())
            }
            Err(StreamError::Closed) => {
                Err(std::io::Error::from(std::io::ErrorKind::ConnectionReset))
            }
            Err(StreamError::LastOperationFailed(err)) => {
                Err(std::io::Error::other(err.to_debug_string()))
            }
        }
    }

    #[inline]
    fn as_async_output_stream(&mut self) -> Option<&mut AsyncOutputStream> {
        Some(self)
    }
}
