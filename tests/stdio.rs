use std::error::Error;
use wstd::io::{AsyncWrite, stderr, stdout};

#[wstd::test]
async fn write_stdout() -> Result<(), Box<dyn Error>> {
    let mut out = stdout();
    out.write_all(b"hello from stdout\n").await?;
    out.flush().await?;
    Ok(())
}

#[wstd::test]
async fn write_stderr() -> Result<(), Box<dyn Error>> {
    let mut err = stderr();
    err.write_all(b"hello from stderr\n").await?;
    err.flush().await?;
    Ok(())
}
