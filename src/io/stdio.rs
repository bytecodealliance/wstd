use super::{AsyncInputStream, AsyncOutputStream, AsyncRead, AsyncWrite, Result};
use std::cell::LazyCell;
#[cfg(target_env = "p3")]
use std::pin::Pin;

#[cfg(target_env = "p2")]
use wasip2::cli::{terminal_input::TerminalInput, terminal_output::TerminalOutput};
#[cfg(target_env = "p3")]
use wasip3::{
    cli::{terminal_input::TerminalInput, terminal_output::TerminalOutput, types::ErrorCode},
    wit_bindgen::FutureRead,
};

#[cfg(target_env = "p3")]
type Completion = FutureRead<std::result::Result<(), ErrorCode>>;

#[cfg(target_env = "p3")]
fn into_io_error(error: ErrorCode) -> std::io::Error {
    let kind = match error {
        ErrorCode::Io => std::io::ErrorKind::Other,
        ErrorCode::IllegalByteSequence => std::io::ErrorKind::InvalidData,
        ErrorCode::Pipe => std::io::ErrorKind::BrokenPipe,
    };
    std::io::Error::new(kind, format!("WASI CLI error: {error:?}"))
}

/// Use the program's stdin as an `AsyncInputStream`.
#[cfg_attr(target_env = "p2", derive(Debug))]
pub struct Stdin {
    stream: AsyncInputStream,
    #[cfg(target_env = "p3")]
    completion: Pin<Box<Completion>>,
    terminput: LazyCell<Option<TerminalInput>>,
}

/// Get the program's stdin for use as an `AsyncInputStream`.
#[cfg(target_env = "p2")]
pub fn stdin() -> Stdin {
    let stream = AsyncInputStream::new(wasip2::cli::stdin::get_stdin());
    Stdin {
        stream,
        terminput: LazyCell::new(wasip2::cli::terminal_stdin::get_terminal_stdin),
    }
}

#[cfg(target_env = "p3")]
pub fn stdin() -> Stdin {
    let (stream, completion) = wasip3::cli::stdin::read_via_stream();
    Stdin {
        stream: AsyncInputStream::new(stream),
        completion: Box::pin(completion.into_future()),
        terminput: LazyCell::new(wasip3::cli::terminal_stdin::get_terminal_stdin),
    }
}

impl Stdin {
    /// Check if stdin is a terminal.
    pub fn is_terminal(&self) -> bool {
        LazyCell::force(&self.terminput).is_some()
    }

    /// Get the `AsyncInputStream` used to implement `Stdin`
    pub fn into_inner(self) -> AsyncInputStream {
        self.stream
    }

    #[cfg(target_env = "p3")]
    async fn check_error(&mut self) -> Result<()> {
        let (stream, completion) = wasip3::cli::stdin::read_via_stream();
        let old_stream = std::mem::replace(&mut self.stream, AsyncInputStream::new(stream));
        drop(old_stream);
        let result = self.completion.as_mut().await.map_err(into_io_error);
        self.completion = Box::pin(completion.into_future());
        result
    }
}

#[cfg(target_env = "p3")]
impl std::fmt::Debug for Stdin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Stdin")
            .field("stream", &self.stream)
            .field("terminput", &self.terminput)
            .finish_non_exhaustive()
    }
}

impl AsyncRead for Stdin {
    #[inline]
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let read = self.stream.read(buf).await?;
        #[cfg(target_env = "p3")]
        if read == 0 && buf.is_empty() {
            self.check_error().await?;
        }
        Ok(read)
    }

    async fn read_to_end(&mut self, buf: &mut Vec<u8>) -> Result<usize> {
        self.stream.read_to_end(buf).await
    }

    #[inline]
    fn as_async_input_stream(&mut self) -> Option<&mut AsyncInputStream> {
        Some(&mut self.stream)
    }
}

/// Use the program's stdout as an `AsyncOutputStream`.
#[cfg_attr(target_env = "p2", derive(Debug))]
pub struct Stdout {
    stream: AsyncOutputStream,
    #[cfg(target_env = "p3")]
    completion: Pin<Box<Completion>>,
    termoutput: LazyCell<Option<TerminalOutput>>,
}

/// Get the program's stdout for use as an `AsyncOutputStream`.
#[cfg(target_env = "p2")]
pub fn stdout() -> Stdout {
    let stream = AsyncOutputStream::new(wasip2::cli::stdout::get_stdout());
    Stdout {
        stream,
        termoutput: LazyCell::new(wasip2::cli::terminal_stdout::get_terminal_stdout),
    }
}

#[cfg(target_env = "p3")]
pub fn stdout() -> Stdout {
    let (tx, rx) = wasip3::wit_stream::new();
    let completion = wasip3::cli::stdout::write_via_stream(rx);
    Stdout {
        stream: AsyncOutputStream::new(tx),
        completion: Box::pin(completion.into_future()),
        termoutput: LazyCell::new(wasip3::cli::terminal_stdout::get_terminal_stdout),
    }
}

impl Stdout {
    /// Check if stdout is a terminal.
    pub fn is_terminal(&self) -> bool {
        LazyCell::force(&self.termoutput).is_some()
    }

    /// Get the `AsyncOutputStream` used to implement `Stdout`
    pub fn into_inner(self) -> AsyncOutputStream {
        self.stream
    }

    #[cfg(target_env = "p3")]
    async fn flush(&mut self) -> Result<()> {
        let Self {
            stream, completion, ..
        } = std::mem::replace(self, stdout());
        drop(stream);

        completion.await.map_err(into_io_error)
    }
}

#[cfg(target_env = "p3")]
impl std::fmt::Debug for Stdout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Stdout")
            .field("stream", &self.stream)
            .field("termoutput", &self.termoutput)
            .finish_non_exhaustive()
    }
}

impl AsyncWrite for Stdout {
    #[inline]
    async fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.stream.write(buf).await
    }

    #[inline]
    async fn flush(&mut self) -> Result<()> {
        #[cfg(target_env = "p2")]
        {
            self.stream.flush().await
        }
        #[cfg(target_env = "p3")]
        {
            Self::flush(self).await
        }
    }

    #[inline]
    async fn write_all(&mut self, buf: &[u8]) -> Result<()> {
        self.stream.write_all(buf).await
    }

    #[inline]
    fn as_async_output_stream(&mut self) -> Option<&mut AsyncOutputStream> {
        self.stream.as_async_output_stream()
    }
}

/// Use the program's stdout as an `AsyncOutputStream`.
#[cfg_attr(target_env = "p2", derive(Debug))]
pub struct Stderr {
    stream: AsyncOutputStream,
    #[cfg(target_env = "p3")]
    completion: Pin<Box<Completion>>,
    termoutput: LazyCell<Option<TerminalOutput>>,
}

/// Get the program's stdout for use as an `AsyncOutputStream`.
#[cfg(target_env = "p2")]
pub fn stderr() -> Stderr {
    let stream = AsyncOutputStream::new(wasip2::cli::stderr::get_stderr());
    Stderr {
        stream,
        termoutput: LazyCell::new(wasip2::cli::terminal_stderr::get_terminal_stderr),
    }
}

#[cfg(target_env = "p3")]
pub fn stderr() -> Stderr {
    let (tx, rx) = wasip3::wit_stream::new();
    let completion = wasip3::cli::stderr::write_via_stream(rx);
    Stderr {
        stream: AsyncOutputStream::new(tx),
        completion: Box::pin(completion.into_future()),
        termoutput: LazyCell::new(wasip3::cli::terminal_stderr::get_terminal_stderr),
    }
}

impl Stderr {
    /// Check if stderr is a terminal.
    pub fn is_terminal(&self) -> bool {
        LazyCell::force(&self.termoutput).is_some()
    }

    /// Get the `AsyncOutputStream` used to implement `Stderr`
    pub fn into_inner(self) -> AsyncOutputStream {
        self.stream
    }

    #[cfg(target_env = "p3")]
    async fn flush(&mut self) -> Result<()> {
        let Self {
            stream, completion, ..
        } = std::mem::replace(self, stderr());
        drop(stream);

        completion.await.map_err(into_io_error)
    }
}

#[cfg(target_env = "p3")]
impl std::fmt::Debug for Stderr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Stderr")
            .field("stream", &self.stream)
            .field("termoutput", &self.termoutput)
            .finish_non_exhaustive()
    }
}

impl AsyncWrite for Stderr {
    #[inline]
    async fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.stream.write(buf).await
    }

    #[inline]
    async fn flush(&mut self) -> Result<()> {
        #[cfg(target_env = "p2")]
        {
            self.stream.flush().await
        }
        #[cfg(target_env = "p3")]
        {
            Self::flush(self).await
        }
    }

    #[inline]
    async fn write_all(&mut self, buf: &[u8]) -> Result<()> {
        self.stream.write_all(buf).await
    }

    #[inline]
    fn as_async_output_stream(&mut self) -> Option<&mut AsyncOutputStream> {
        self.stream.as_async_output_stream()
    }
}

#[cfg(test)]
mod test {
    use crate::io::AsyncWrite;
    use crate::runtime::block_on;
    #[test]
    // No internal predicate. Run test with --nocapture and inspect output manually.
    fn stdout_println_hello_world() {
        block_on(async {
            let mut stdout = super::stdout();
            let term = if stdout.is_terminal() { "is" } else { "is not" };
            stdout
                .write_all(format!("hello, world! stdout {term} a terminal\n",).as_bytes())
                .await
                .unwrap();
            stdout.flush().await.unwrap();
        })
    }
    #[test]
    // No internal predicate. Run test with --nocapture and inspect output manually.
    fn stderr_println_hello_world() {
        block_on(async {
            let mut stderr = super::stderr();
            let term = if stderr.is_terminal() { "is" } else { "is not" };
            stderr
                .write_all(format!("hello, world! stderr {term} a terminal\n",).as_bytes())
                .await
                .unwrap();
            stderr.flush().await.unwrap();
        })
    }
}
