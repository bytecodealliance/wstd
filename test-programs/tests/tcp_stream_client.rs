use anyhow::{Context, Result};
use std::net::{Shutdown, TcpListener};
use std::process::{Command, Stdio};

fn run(component: &str, p3: bool) -> Result<()> {
    use std::io::{Read, Write};

    let server = TcpListener::bind("127.0.0.1:0").context("binding temporary test server")?;
    let addr = server
        .local_addr()
        .context("getting local listener address")?;

    let mut command = Command::new("wasmtime");
    command.arg("run").arg("-Sinherit-network");
    if p3 {
        command.arg("-Sp3");
    }
    let child = command
        .arg(component)
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

#[test_log::test]
fn tcp_stream_client_p2() -> Result<()> {
    run(test_programs::TCP_STREAM_CLIENT, false)
}

#[test_log::test]
fn tcp_stream_client_p3() -> Result<()> {
    if test_programs::NIGHTLY_TOOLCHAIN {
        run(test_programs::TCP_STREAM_CLIENT_P3, true)?;
    }
    Ok(())
}
