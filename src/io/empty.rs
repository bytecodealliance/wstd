use super::{AsyncRead, AsyncWrite};

#[non_exhaustive]
pub struct Empty;

#[async_trait::async_trait(?Send)]
impl AsyncRead for Empty {
    async fn read(&mut self, _buf: &mut [u8]) -> super::Result<usize> {
        Ok(0)
    }
}

#[async_trait::async_trait(?Send)]
impl AsyncWrite for Empty {
    async fn write(&mut self, buf: &[u8]) -> super::Result<usize> {
        Ok(buf.len())
    }

    async fn flush(&mut self) -> super::Result<()> {
        Ok(())
    }
}

/// Creates a value that is always at EOF for reads, and ignores all data written.
pub fn empty() -> Empty {
    Empty {}
}
