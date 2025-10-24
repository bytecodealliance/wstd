use anyhow::{Context, Result};
use std::net::{Shutdown, TcpListener};
use std::process::{Command, Stdio};

#[test_log::test]
fn tcp_stream_client() -> Result<()> {
    use std::io::{Read, Write};

    let server = TcpListener::bind("127.0.0.1:8082").context("binding temporary test server")?;
    let addr = server
        .local_addr()
        .context("getting local listener address")?;

    let child = Command::new("wasmtime")
        .arg("run")
        .arg("-Sinherit-network")
        .arg(test_programs::TCP_STREAM_CLIENT)
        .arg(addr.to_string())
        .stdout(Stdio::piped())
        .spawn()
        .context("spawning wasmtime component")?;

    let (mut server_stream, _addr) = server
        .accept()
        .context("accepting TCP connection from component")?;

    let mut buf = [0u8; 5];
    server_stream
        .read_exact(&mut buf)
        .context("reading ping message")?;
    assert_eq!(&buf, b"ping\n", "expected ping from component");

    server_stream
        .write_all(b"pong\n")
        .context("writing reply")?;
    server_stream.flush().context("flushing")?;

    server_stream
        .shutdown(Shutdown::Both)
        .context("shutting down connection")?;

    let output = child
        .wait_with_output()
        .context("waiting for component exit")?;

    assert!(
        output.status.success(),
        "\nComponent exited abnormally (stderr:\n{})",
        String::from_utf8_lossy(&output.stderr)
    );

    Ok(())
}
