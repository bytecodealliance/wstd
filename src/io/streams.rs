use super::{AsyncPollable, AsyncRead, AsyncWrite};
use std::cell::OnceCell;
use std::io::Result;
use wasip2::io::streams::{InputStream, OutputStream, StreamError};

/// A wrapper for WASI's `InputStream` resource that provides implementations of `AsyncRead` and
/// `AsyncPollable`.
#[derive(Debug)]
pub struct AsyncInputStream {
    // Lazily initialized pollable, used for lifetime of stream to check readiness.
    // Field ordering matters: this child must be dropped before stream
    subscription: OnceCell<AsyncPollable>,
    stream: InputStream,
}

impl AsyncInputStream {
    /// Construct an `AsyncInputStream` from a WASI `InputStream` resource.
    pub fn new(stream: InputStream) -> Self {
        Self {
            subscription: OnceCell::new(),
            stream,
        }
    }
    /// Await for read readiness.
    async fn ready(&self) {
        // Lazily initialize the AsyncPollable
        let subscription = self
            .subscription
            .get_or_init(|| AsyncPollable::new(self.stream.subscribe()));
        // Wait on readiness
        subscription.wait_for().await;
    }
    /// Asynchronously read from the input stream.
    /// This method is the same as [`AsyncRead::read`], but doesn't require a `&mut self`.
    pub async fn read(&self, buf: &mut [u8]) -> Result<usize> {
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
                    return Err(std::io::Error::other(err.to_debug_string()))
                }
            }
        };
        let len = read.len();
        buf[0..len].copy_from_slice(&read);
        Ok(len)
    }
}

impl AsyncRead for AsyncInputStream {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        Self::read(self, buf).await
    }

    #[inline]
    fn as_async_input_stream(&self) -> Option<&AsyncInputStream> {
        Some(self)
    }
}

/// A wrapper for WASI's `output-stream` resource that provides implementations of `AsyncWrite` and
/// `AsyncPollable`.
#[derive(Debug)]
pub struct AsyncOutputStream {
    // Lazily initialized pollable, used for lifetime of stream to check readiness.
    // Field ordering matters: this child must be dropped before stream
    subscription: OnceCell<AsyncPollable>,
    stream: OutputStream,
}

impl AsyncOutputStream {
    /// Construct an `AsyncOutputStream` from a WASI `OutputStream` resource.
    pub fn new(stream: OutputStream) -> Self {
        Self {
            subscription: OnceCell::new(),
            stream,
        }
    }
    /// Await write readiness.
    async fn ready(&self) {
        // Lazily initialize the AsyncPollable
        let subscription = self
            .subscription
            .get_or_init(|| AsyncPollable::new(self.stream.subscribe()));
        // Wait on readiness
        subscription.wait_for().await;
    }
    /// Asynchronously write to the output stream. This method is the same as
    /// [`AsyncWrite::write`], but doesn't require a `&mut self`.
    ///
    /// Awaits for write readiness, and then performs at most one write to the
    /// output stream. Returns how much of the argument `buf` was written, or
    /// a `std::io::Error` indicating either an error returned by the stream write
    /// using the debug string provided by the WASI error, or else that the,
    /// indicated by `std::io::ErrorKind::ConnectionReset`.
    pub async fn write(&self, buf: &[u8]) -> Result<usize> {
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
                            return Err(std::io::Error::from(std::io::ErrorKind::ConnectionReset))
                        }
                        Err(StreamError::LastOperationFailed(err)) => {
                            return Err(std::io::Error::other(err.to_debug_string()))
                        }
                    }
                }
                Err(StreamError::Closed) => {
                    return Err(std::io::Error::from(std::io::ErrorKind::ConnectionReset))
                }
                Err(StreamError::LastOperationFailed(err)) => {
                    return Err(std::io::Error::other(err.to_debug_string()))
                }
            }
        }
    }
    /// Asyncronously flush the output stream. Initiates a flush, and then
    /// awaits until the flush is complete and the output stream is ready for
    /// writing again.
    ///
    /// This method is the same as [`AsyncWrite::flush`], but doesn't require
    /// a `&mut self`.
    ///
    /// Fails with a `std::io::Error` indicating either an error returned by
    /// the stream flush, using the debug string provided by the WASI error,
    /// or else that the stream is closed, indicated by
    /// `std::io::ErrorKind::ConnectionReset`.
    pub async fn flush(&self) -> Result<()> {
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
}
impl AsyncWrite for AsyncOutputStream {
    // Required methods
    async fn write(&mut self, buf: &[u8]) -> Result<usize> {
        Self::write(self, buf).await
    }
    async fn flush(&mut self) -> Result<()> {
        Self::flush(self).await
    }

    #[inline]
    fn as_async_output_stream(&self) -> Option<&AsyncOutputStream> {
        Some(self)
    }
}

/// Wait for both streams to be ready and then do a WASI splice.
pub(crate) async fn splice(
    reader: &AsyncInputStream,
    writer: &AsyncOutputStream,
    len: u64,
) -> core::result::Result<u64, StreamError> {
    // Wait for both streams to be ready.
    let r = reader.ready();
    writer.ready().await;
    r.await;

    writer.stream.splice(&reader.stream, len)
}
