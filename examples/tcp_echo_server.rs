use wstd::io;
use wstd::iter::AsyncIterator;
use wstd::net::TcpListener;

#[wstd::main]
async fn main() -> io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    println!("Listening on {}", listener.local_addr()?);
    println!("type `nc localhost 8080` to create a TCP client");

    let mut incoming = listener.incoming();
    while let Some(stream) = incoming.next().await {
        let stream = stream?;
        println!("Accepted from: {}", stream.peer_addr()?);
        wstd::runtime::spawn(async move {
            // If echo copy fails, we can ignore it.
            let _ = io::copy(&stream, &stream).await;
        })
        .detach();
    }
    Ok(())
}
