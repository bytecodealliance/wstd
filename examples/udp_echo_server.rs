#![cfg_attr(not(target_os = "wasi"), no_main)]
#![cfg(target_os = "wasi")]

use wstd::io;
use wstd::net::UdpSocket;

#[wstd::main]
async fn main() -> io::Result<()> {
    let socket = UdpSocket::bind("127.0.0.1:0").await?;
    println!("Listening on {}", socket.local_addr()?);
    println!("type `nc -u localhost <PORT>` to create a UDP client");

    let mut buf = vec![0; 65535];
    loop {
        let (len, peer) = socket.recv_from(&mut buf).await?;
        println!("Received {len} bytes from: {peer}");
        // If the echo send fails, we can ignore it: one socket serves every
        // peer here, so a failure for one must not end the loop for the rest.
        let _ = socket.send_to(&buf[..len], peer).await;
    }
}
