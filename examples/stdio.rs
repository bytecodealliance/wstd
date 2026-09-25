#![cfg_attr(not(target_os = "wasi"), no_main)]
#![cfg(target_os = "wasi")]

//! Demonstrates async line-oriented stdin, stdout, stderr, and flushing.

use anyhow::Result;
use wstd::io::{AsyncRead, AsyncWrite};

struct LineReader<R> {
    reader: R,
    buffer: Vec<u8>,
}

impl<R: AsyncRead> LineReader<R> {
    fn new(reader: R) -> Self {
        Self {
            reader,
            buffer: Vec::new(),
        }
    }

    async fn read_line(&mut self) -> std::io::Result<Option<Vec<u8>>> {
        loop {
            if let Some(newline) = self.buffer.iter().position(|byte| *byte == b'\n') {
                return Ok(Some(self.buffer.drain(..=newline).collect()));
            }

            let mut chunk = [0; 1024];
            match self.reader.read(&mut chunk).await? {
                0 if self.buffer.is_empty() => return Ok(None),
                0 => return Ok(Some(std::mem::take(&mut self.buffer))),
                len => {
                    self.buffer.extend_from_slice(&chunk[..len]);
                }
            }
        }
    }
}

#[wstd::main]
async fn main() -> Result<()> {
    let mut stdin = LineReader::new(wstd::io::stdin());
    let mut stdout = wstd::io::stdout();
    let mut stderr = wstd::io::stderr();

    let mut stdin_error = None;
    while let Some(line) = match stdin.read_line().await {
        Ok(Some(line)) => Some(line),
        Ok(None) => {
            stdin_error = Some(std::io::ErrorKind::UnexpectedEof.into());
            None
        }
        Err(error) => {
            stdin_error = Some(error);
            None
        }
    } {
        stdout.write_all(b"stdout: ").await.unwrap();
        stdout.write_all(&line).await.unwrap();
        stdout.flush().await.unwrap();

        stderr.write_all(b"stderr: ").await.unwrap();
        stderr.write_all(&line).await.unwrap();
        stderr.flush().await.unwrap();
    }
    let stdin_error = stdin_error.expect("the loop only exits after a stdin error");

    let stdout_result = match stdout.write_all(b"stdin closed\n").await {
        Ok(()) => stdout.flush().await,
        Err(error) => Err(error),
    };

    stderr
        .write_all(format!("stdin error: {:?}\n", stdin_error.kind()).as_bytes())
        .await?;
    if let Err(error) = stdout_result {
        stderr
            .write_all(format!("stdout error: {:?}\n", error.kind()).as_bytes())
            .await?;
    }
    stderr.flush().await?;

    Ok(())
}
