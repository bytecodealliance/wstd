use anyhow::{Context, Result};
use std::net::UdpSocket;
use std::process::{Command, Stdio};
use std::time::Duration;

fn run(component: &str, p3: bool) -> Result<()> {
    // Port 0: the host picks a free port, which the component is told about
    // by argument, so this test can't collide with anything else running.
    let server = UdpSocket::bind("127.0.0.1:0").context("binding temporary test server")?;
    // Without a deadline, a dropped datagram would hang the test suite rather
    // than fail it.
    server
        .set_read_timeout(Some(Duration::from_secs(10)))
        .context("setting server read timeout")?;
    let addr = server
        .local_addr()
        .context("getting local server address")?;

    let mut command = Command::new("wasmtime");
    command.arg("run").arg("-Sinherit-network");
    if p3 {
        command.arg("-Sp3");
    }
    let child = command
        .arg(component)
        .arg(addr.to_string())
        .stderr(Stdio::piped())
        .spawn()
        .context("spawning wasmtime component")?;

    // The component exercises both `UdpStream::connect` and converting a bound
    // `UdpSocket` with `UdpSocket::connect`.
    for _ in 0..2 {
        let mut buf = [0; 5];
        let (len, component_addr) = server
            .recv_from(&mut buf)
            .context("receiving ping datagram from component")?;
        assert_eq!(&buf[..len], b"ping\n", "expected ping from component");

        // The component uses a five-byte buffer, so the remaining bytes should
        // be discarded with the rest of this datagram.
        server
            .send_to(b"pong\nignored", component_addr)
            .context("writing reply")?;
    }

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
fn udp_stream_client_p2() -> Result<()> {
    run(test_programs::UDP_STREAM_CLIENT, false)
}

#[test_log::test]
fn udp_stream_client_p3() -> Result<()> {
    if test_programs::NIGHTLY_TOOLCHAIN {
        run(test_programs::UDP_STREAM_CLIENT_P3, true)?;
    }
    Ok(())
}
