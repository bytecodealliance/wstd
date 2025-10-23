use wstd::io::{self, AsyncRead, AsyncWrite};
use wstd::net::TcpStream;

#[wstd::main]
async fn main() -> io::Result<()> {
    let mut args = std::env::args();

    let _ = args.next();

    let addr = args.next().ok_or_else(|| {
        io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "address argument required",
        )
    })?;

    let mut stream = TcpStream::connect(addr).await?;

    stream.write_all(b"ping\n").await?;

    let mut reply = Vec::new();
    stream.read_to_end(&mut reply).await?;

    Ok(())
}
