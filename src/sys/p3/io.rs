use crate::io::{AsyncRead, AsyncWrite};
use std::pin::Pin;
use std::task::{Context, Poll};
use wasip3::wit_bindgen::rt::async_support::{StreamReader, StreamResult, StreamWriter};

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

impl futures_lite::stream::Stream for AsyncInputChunkStream {
    type Item = Result<Vec<u8>, std::io::Error>;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        let read_buf = Vec::with_capacity(this.chunk_size);
        let mut fut = std::pin::pin!(this.stream.reader.read(read_buf));
        match fut.as_mut().poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready((result, data)) => match result {
                StreamResult::Complete(_) if data.is_empty() => Poll::Pending,
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
        Ok(())
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
