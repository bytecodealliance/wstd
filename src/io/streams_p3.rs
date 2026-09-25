use super::{AsyncRead, AsyncWrite};

use wasip3::wit_bindgen::{StreamReader, StreamResult, StreamWriter};

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

/// A wrapper for the readable end of a `stream<u8>` that provides an
/// implementation of `AsyncRead`.
#[derive(Debug)]
pub struct AsyncInputStream {
    stream: StreamReader<u8>,
}

impl AsyncInputStream {
    pub fn new(stream: StreamReader<u8>) -> Self {
        Self { stream }
    }

    /// Move the entire contents of an input stream directly into an output
    /// stream, until the input stream has closed. This operation is optimized
    /// to avoid copying stream contents into and out of memory.
    pub async fn copy_to(&mut self, writer: &mut AsyncOutputStream) -> std::io::Result<u64> {
        // TODO: The current implementation avoids the extra copy within Wasm
        // that occurs with `AsyncRead::read`, but it still requires the host to
        // copy bytes into Wasm in the first place and then run guest code to
        // move those bytes to the output stream. This should all be further
        // optimized away by switching to `stream.forward`.
        const CHUNK_SIZE: usize = 1024;
        let mut vec = Vec::with_capacity(CHUNK_SIZE);
        let mut written = 0;
        loop {
            let (result, new_vec) = self.stream.read(vec).await;
            vec = new_vec;
            match result {
                StreamResult::Complete(r) => {
                    writer.write_all(&vec).await?;
                    written += r as u64;
                }
                StreamResult::Dropped => break,
                StreamResult::Cancelled => return Err(std::io::ErrorKind::Interrupted.into()),
            }
            vec.clear();
        }
        Ok(written)
    }

    /// Use this `AsyncInputStream` as a `futures_lite::stream::Stream` with
    /// items of `Result<Vec<u8>, std::io::Error>`. The returned byte vectors
    /// will be at most 8k. If you want to control chunk size, use
    /// `Self::into_stream_of`.
    pub fn into_stream(self) -> AsyncInputChunkStream {
        AsyncInputChunkStream {
            state: AsyncInputChunkStreamState::Ready(self),
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
            state: AsyncInputChunkStreamState::Ready(self),
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
        let vec = Vec::with_capacity(buf.len());
        let (result, vec) = self.stream.read(vec).await;
        match result {
            StreamResult::Complete(_) => {
                buf[..vec.len()].copy_from_slice(&vec);
                Ok(vec.len())
            }
            StreamResult::Dropped => Ok(0),
            StreamResult::Cancelled => Err(std::io::ErrorKind::Interrupted.into()),
        }
    }

    #[inline]
    fn as_async_input_stream(&mut self) -> Option<&mut AsyncInputStream> {
        Some(self)
    }
}

/// Wrapper of `AsyncInputStream` that impls `futures_lite::stream::Stream`
/// with an item of `Result<Vec<u8>, std::io::Error>`
pub struct AsyncInputChunkStream {
    state: AsyncInputChunkStreamState,
    chunk_size: usize,
}

enum AsyncInputChunkStreamState {
    Ready(AsyncInputStream),
    Reading(Pin<Box<dyn Future<Output = AsyncInputChunkReadResult>>>),
    Done,
}

enum AsyncInputChunkReadResult {
    Chunk {
        chunk: Vec<u8>,
        stream: AsyncInputStream,
    },
    Done(std::io::Result<()>),
}

impl AsyncInputChunkStream {
    /// Extract the `AsyncInputStream` which backs this stream. The operation
    /// will fail if a read is currently in progress or the stream is done in
    /// which case `self` is returned.
    pub fn into_inner(self) -> Result<AsyncInputStream, Self> {
        match self.state {
            AsyncInputChunkStreamState::Ready(stream) => Ok(stream),
            AsyncInputChunkStreamState::Reading(_) | AsyncInputChunkStreamState::Done => Err(self),
        }
    }
}

impl futures_lite::stream::Stream for AsyncInputChunkStream {
    type Item = Result<Vec<u8>, std::io::Error>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            match &mut self.state {
                AsyncInputChunkStreamState::Ready(_) => {
                    // We need to take ownership of the stream before we setting
                    // the new state, which requires this temporary `replace`.
                    let AsyncInputChunkStreamState::Ready(mut stream) =
                        std::mem::replace(&mut self.state, AsyncInputChunkStreamState::Done)
                    else {
                        unreachable!();
                    };
                    let chunk_size = self.chunk_size;
                    self.state = AsyncInputChunkStreamState::Reading(Box::pin(async move {
                        let (result, chunk) =
                            stream.stream.read(Vec::with_capacity(chunk_size)).await;
                        match result {
                            StreamResult::Complete(_) => {
                                AsyncInputChunkReadResult::Chunk { chunk, stream }
                            }
                            StreamResult::Dropped => AsyncInputChunkReadResult::Done(Ok(())),
                            StreamResult::Cancelled => AsyncInputChunkReadResult::Done(Err(
                                std::io::ErrorKind::Interrupted.into(),
                            )),
                        }
                    }));
                }
                AsyncInputChunkStreamState::Reading(read) => {
                    match std::task::ready!(read.as_mut().poll(cx)) {
                        AsyncInputChunkReadResult::Chunk { chunk, stream } => {
                            self.state = AsyncInputChunkStreamState::Ready(stream);
                            return Poll::Ready(Some(Ok(chunk)));
                        }
                        AsyncInputChunkReadResult::Done(result) => {
                            self.state = AsyncInputChunkStreamState::Done;
                            return match result {
                                Ok(()) => Poll::Ready(None),
                                Err(error) => Poll::Ready(Some(Err(error))),
                            };
                        }
                    }
                }
                AsyncInputChunkStreamState::Done => return Poll::Ready(None),
            }
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
    /// the byte stream. If a `read` is in progress or the stream is completed
    /// this will error and return back `self`.
    pub fn into_inner(self) -> Result<(AsyncInputStream, Vec<u8>), Self> {
        match self.stream.into_inner() {
            Ok(inner) => Ok((
                inner,
                self.buffer
                    .collect::<Result<Vec<u8>, std::io::Error>>()
                    .expect("read of Cursor<Vec<u8>> is infallible"),
            )),
            Err(stream) => Err(Self {
                stream,
                buffer: self.buffer,
            }),
        }
    }
}

impl futures_lite::stream::Stream for AsyncInputByteStream {
    type Item = std::result::Result<u8, std::io::Error>;
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

/// A wrapper for the writable end of a `stream<u8>` resource that provides
/// implementations of `AsyncWrite`.
#[derive(Debug)]
pub struct AsyncOutputStream {
    stream: StreamWriter<u8>,
}

impl AsyncOutputStream {
    pub fn new(stream: StreamWriter<u8>) -> Self {
        Self { stream }
    }
}

impl AsyncWrite for AsyncOutputStream {
    /// Asynchronously write to the output stream.
    ///
    /// Performs at most one write to the output stream. Returns how much of the
    /// argument `buf` was written, or a `std::io::Error`.
    async fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut vec = Vec::with_capacity(buf.len());
        vec.extend_from_slice(buf);
        let (result, _abi_buf) = self.stream.write(vec).await;
        match result {
            StreamResult::Complete(sent) => Ok(sent),
            StreamResult::Dropped => Err(std::io::ErrorKind::ConnectionReset.into()),
            StreamResult::Cancelled => unreachable!("Write operation cannot be cancelled"),
        }
    }

    async fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
        let remaining = self.stream.write_all(buf.to_vec()).await;
        if remaining.is_empty() {
            Ok(())
        } else {
            Err(std::io::ErrorKind::ConnectionReset.into())
        }
    }

    /// # Warning
    ///
    /// This is a no-op on generic p3 streams. Use interface-specific flush
    /// methods when available (e.g. [`crate::io::Stdout::flush`] or
    /// [`crate::io::Stderr::flush`]).
    async fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }

    #[inline]
    fn as_async_output_stream(&mut self) -> Option<&mut AsyncOutputStream> {
        Some(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_lite::StreamExt;

    #[test]
    fn chunk_stream_partial_read_works() {
        crate::runtime::block_on(async {
            let (mut writer, reader) = wasip3::wit_stream::new();
            let read = crate::runtime::spawn(async move {
                let mut chunks = AsyncInputStream::new(reader).into_stream_of(4);
                let chunk = chunks.next().await.unwrap().unwrap();
                let end = chunks.next().await;
                (chunk, end)
            });

            assert!(writer.write_all(vec![1, 2]).await.is_empty());
            drop(writer);

            let (chunk, end) = read.await;
            assert_eq!(chunk, [1, 2]);
            assert!(end.is_none());
        });
    }

    #[test]
    fn output_stream_reports_closed_reader() {
        crate::runtime::block_on(async {
            let (writer, reader) = wasip3::wit_stream::new();
            drop(reader);

            let error = AsyncOutputStream::new(writer)
                .write(&[1])
                .await
                .unwrap_err();

            assert_eq!(error.kind(), std::io::ErrorKind::ConnectionReset);
        });
    }

    #[test]
    fn output_stream_write_all_sends_entire_buffer() {
        crate::runtime::block_on(async {
            let (writer, reader) = wasip3::wit_stream::new();
            let collect = crate::runtime::spawn(reader.collect());
            let expected: Vec<_> = (0..=255).collect();
            let mut output = AsyncOutputStream::new(writer);

            output.write_all(&expected).await.unwrap();
            drop(output);

            assert_eq!(collect.await, expected);
        });
    }

    #[test]
    fn copy_to_works() {
        crate::runtime::block_on(async {
            let (mut source_writer, source_reader) = wasip3::wit_stream::new();
            let (destination_writer, destination_reader) = wasip3::wit_stream::new();

            let copy = crate::runtime::spawn(async move {
                let mut source = AsyncInputStream::new(source_reader);
                let mut destination = AsyncOutputStream::new(destination_writer);
                let copied = source.copy_to(&mut destination).await.unwrap();
                drop(destination);
                copied
            });
            let collect = crate::runtime::spawn(destination_reader.collect());

            assert!(source_writer.write_all(vec![1]).await.is_empty());
            assert!(source_writer.write_all(vec![2]).await.is_empty());
            assert!(source_writer.write_all(vec![3, 4, 5]).await.is_empty());
            drop(source_writer);

            let copied = copy.await;
            let actual = collect.await;

            assert_eq!(copied, 5);
            assert_eq!(actual, [1, 2, 3, 4, 5]);
        });
    }
}
