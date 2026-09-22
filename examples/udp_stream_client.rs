#![cfg_attr(not(target_os = "wasi"), no_main)]
#![cfg(target_os = "wasi")]

use wstd::io;
use wstd::net::{UdpSocket, UdpStream};

async fn ping(stream: &UdpStream) -> io::Result<()> {
    assert_eq!(stream.send(b"ping\n").await?, 5);

    let mut reply = [0; 5];
    let len = stream.recv(&mut reply).await?;
    assert_eq!(&reply[..len], b"pong\n");

    Ok(())
}

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

    let stream = UdpStream::connect(addr).await?;
    ping(&stream).await?;

    let peer = stream.peer_addr()?;
    drop(stream);

    let local_addr = if peer.is_ipv4() {
        "0.0.0.0:0"
    } else {
        "[::]:0"
    };
    let stream = UdpSocket::bind(local_addr).await?.connect(peer)?;
    ping(&stream).await?;

    Ok(())
}
