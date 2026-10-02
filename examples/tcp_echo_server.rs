#![cfg_attr(not(target_os = "wasi"), no_main)]
#![cfg(target_os = "wasi")]

use wstd::io;
use wstd::iter::AsyncIterator;
use wstd::net::TcpListener;

#[wstd::main]
async fn main() -> io::Result<()> {
    let mut listener = TcpListener::bind("127.0.0.1:0").await?;
    println!("Listening on {}", listener.local_addr()?);
    println!("type `nc localhost <PORT>` to create a TCP client");

    let mut incoming = listener.incoming();
    while let Some(stream) = incoming.next().await {
        let stream = stream?;
        println!("Accepted from: {}", stream.peer_addr()?);
        wstd::runtime::spawn(async move {
            // If echo copy fails, we can ignore it.
            let mut stream = stream;
            let (mut read_half, mut write_half) = stream.split();
            let _ = io::copy(&mut read_half, &mut write_half).await;
        })
        .detach();
    }
    Ok(())
}
