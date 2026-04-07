#[cfg(all(feature = "wasip2", not(feature = "wasip3")))]
mod p2 {
    use crate::io::{AsyncPollable, AsyncRead, AsyncWrite};
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
        fn poll_ready(&self, cx: &mut Context<'_>) -> Poll<()> {
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
        async fn ready(&self) {
            poll_fn(|cx| self.poll_ready(cx)).await
        }
        /// Asynchronously read from the input stream.
        /// This method is the same as [`AsyncRead::read`], but doesn't require a `&mut self`.
        pub async fn read(&self, buf: &mut [u8]) -> std::io::Result<usize> {
            let read = loop {
                self.ready().await;
                match self.stream.read(buf.len() as u64) {
                    Ok(r) if r.is_empty() => continue,
                    Ok(r) => break r,
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

        /// Move the entire contents of an input stream directly into an output
        /// stream, until the input stream has closed.
        pub async fn copy_to(&self, writer: &AsyncOutputStream) -> std::io::Result<u64> {
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
        /// items of `Result<Vec<u8>, std::io::Error>`.
        pub fn into_stream(self) -> AsyncInputChunkStream {
            AsyncInputChunkStream {
                stream: self,
                chunk_size: 8 * 1024,
            }
        }

        /// Use this `AsyncInputStream` as a `futures_lite::stream::Stream` with
        /// items of `Result<Vec<u8>, std::io::Error>`. The returned byte vectors
        /// will be at most the `chunk_size` argument specified.
        pub fn into_stream_of(self, chunk_size: usize) -> AsyncInputChunkStream {
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
            Self::read(self, buf).await
        }

        #[inline]
        fn as_async_input_stream(&self) -> Option<&AsyncInputStream> {
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
            match self.stream.poll_ready(cx) {
                Poll::Pending => Poll::Pending,
                Poll::Ready(()) => match self.stream.stream.read(self.chunk_size as u64) {
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
        pub(crate) async fn ready(&self) {
            let subscription = self
                .subscription
                .get_or_init(|| AsyncPollable::new(self.stream.subscribe()));
            subscription.wait_for().await;
        }
        /// Asynchronously write to the output stream. This method is the same as
        /// [`AsyncWrite::write`], but doesn't require a `&mut self`.
        pub async fn write(&self, buf: &[u8]) -> std::io::Result<usize> {
            loop {
                match self.stream.check_write() {
                    Ok(0) => {
                        self.ready().await;
                        continue;
                    }
                    Ok(some) => {
                        let writable = some.try_into().unwrap_or(usize::MAX).min(buf.len());
                        match self.stream.write(&buf[0..writable]) {
                            Ok(()) => return Ok(writable),
                            Err(StreamError::Closed) => {
                                return Err(std::io::Error::from(
                                    std::io::ErrorKind::ConnectionReset,
                                ));
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

        /// Asynchronously write to the output stream. This method is the same as
        /// [`AsyncWrite::write_all`], but doesn't require a `&mut self`.
        pub async fn write_all(&self, buf: &[u8]) -> std::io::Result<()> {
            let mut to_write = &buf[0..];
            loop {
                let bytes_written = self.write(to_write).await?;
                to_write = &to_write[bytes_written..];
                if to_write.is_empty() {
                    return Ok(());
                }
            }
        }

        /// Asyncronously flush the output stream.
        pub async fn flush(&self) -> std::io::Result<()> {
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
        async fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            Self::write(self, buf).await
        }
        async fn flush(&mut self) -> std::io::Result<()> {
            Self::flush(self).await
        }

        #[inline]
        fn as_async_output_stream(&self) -> Option<&AsyncOutputStream> {
            Some(self)
        }
    }
}

#[cfg(all(feature = "wasip2", not(feature = "wasip3")))]
pub use p2::*;

#[cfg(feature = "wasip3")]
mod p3 {
    use crate::io::{AsyncRead, AsyncWrite};
    use std::pin::Pin;
    use std::task::{Context, Poll};
    use wit_bindgen::rt::async_support::{StreamReader, StreamResult, StreamWriter};

    /// A wrapper for a p3 `StreamReader<u8>` that provides `AsyncRead`.
    pub struct AsyncInputStream {
        reader: StreamReader<u8>,
    }

    impl std::fmt::Debug for AsyncInputStream {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("AsyncInputStream").finish()
        }
    }

    impl AsyncInputStream {
        /// Construct an `AsyncInputStream` from a p3 `StreamReader<u8>`.
        pub fn new(reader: StreamReader<u8>) -> Self {
            Self { reader }
        }

        /// Asynchronously read from the input stream.
        pub async fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let read_buf = Vec::with_capacity(buf.len());
            let (result, data) = self.reader.read(read_buf).await;
            match result {
                StreamResult::Complete(_n) => {
                    if data.is_empty() {
                        // Stream ended
                        return Ok(0);
                    }
                    let len = data.len();
                    buf[0..len].copy_from_slice(&data);
                    Ok(len)
                }
                StreamResult::Dropped => Ok(0),
                StreamResult::Cancelled => Ok(0),
            }
        }

        /// Use this `AsyncInputStream` as a `futures_lite::stream::Stream` with
        /// items of `Result<Vec<u8>, std::io::Error>`.
        pub fn into_stream(self) -> AsyncInputChunkStream {
            AsyncInputChunkStream {
                stream: self,
                chunk_size: 8 * 1024,
            }
        }

        /// Use this `AsyncInputStream` as a `futures_lite::stream::Stream` with
        /// items of `Result<Vec<u8>, std::io::Error>`. The returned byte vectors
        /// will be at most the `chunk_size` argument specified.
        pub fn into_stream_of(self, chunk_size: usize) -> AsyncInputChunkStream {
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

        /// Get a reference to the inner `StreamReader<u8>`.
        pub fn inner(&self) -> &StreamReader<u8> {
            &self.reader
        }

        /// Consume this wrapper and return the inner `StreamReader<u8>`.
        pub fn into_inner(self) -> StreamReader<u8> {
            self.reader
        }
    }

    impl AsyncRead for AsyncInputStream {
        async fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            AsyncInputStream::read(self, buf).await
        }

        #[inline]
        fn as_async_input_stream(&self) -> Option<&AsyncInputStream> {
            Some(self)
        }
    }

    /// Wrapper of `AsyncInputStream` that impls `futures_lite::stream::Stream`
    pub struct AsyncInputChunkStream {
        stream: AsyncInputStream,
        chunk_size: usize,
    }

    impl AsyncInputChunkStream {
        pub fn into_inner(self) -> AsyncInputStream {
            self.stream
        }
    }

    // Note: This is not a true poll-based stream for p3. We use a simple async approach.
    // The `poll_next` implementation starts a read and awaits it inline. This works because
    // in p3, the reads are natively async.
    impl futures_lite::stream::Stream for AsyncInputChunkStream {
        type Item = Result<Vec<u8>, std::io::Error>;
        fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            let this = self.get_mut();
            let read_buf = Vec::with_capacity(this.chunk_size);
            let mut fut = std::pin::pin!(this.stream.reader.read(read_buf));
            match fut.as_mut().poll(cx) {
                Poll::Pending => Poll::Pending,
                Poll::Ready((result, data)) => match result {
                    StreamResult::Complete(_) if data.is_empty() => {
                        // Try again, might have more data
                        Poll::Pending
                    }
                    StreamResult::Complete(_) => Poll::Ready(Some(Ok(data))),
                    StreamResult::Dropped => Poll::Ready(None),
                    StreamResult::Cancelled => Poll::Ready(None),
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

    /// A wrapper for a p3 `StreamWriter<u8>` that provides `AsyncWrite`.
    pub struct AsyncOutputStream {
        writer: StreamWriter<u8>,
    }

    impl std::fmt::Debug for AsyncOutputStream {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("AsyncOutputStream").finish()
        }
    }

    impl AsyncOutputStream {
        /// Construct an `AsyncOutputStream` from a p3 `StreamWriter<u8>`.
        pub fn new(writer: StreamWriter<u8>) -> Self {
            Self { writer }
        }

        /// Asynchronously write to the output stream.
        pub async fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            let data = buf.to_vec();
            let remaining = self.writer.write_all(data).await;
            Ok(buf.len() - remaining.len())
        }

        /// Asynchronously write all bytes to the output stream.
        pub async fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
            let data = buf.to_vec();
            let remaining = self.writer.write_all(data).await;
            if remaining.is_empty() {
                Ok(())
            } else {
                Err(std::io::Error::from(std::io::ErrorKind::ConnectionReset))
            }
        }

        /// Flush the output stream (no-op for p3 streams).
        pub async fn flush(&mut self) -> std::io::Result<()> {
            // p3 streams don't have an explicit flush
            Ok(())
        }

        /// Get a mutable reference to the inner `StreamWriter<u8>`.
        pub fn inner_mut(&mut self) -> &mut StreamWriter<u8> {
            &mut self.writer
        }

        /// Consume this wrapper and return the inner `StreamWriter<u8>`.
        pub fn into_inner(self) -> StreamWriter<u8> {
            self.writer
        }
    }

    impl AsyncWrite for AsyncOutputStream {
        async fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            AsyncOutputStream::write(self, buf).await
        }
        async fn flush(&mut self) -> std::io::Result<()> {
            AsyncOutputStream::flush(self).await
        }

        #[inline]
        fn as_async_output_stream(&self) -> Option<&AsyncOutputStream> {
            Some(self)
        }
    }
}

#[cfg(feature = "wasip3")]
pub use p3::*;
