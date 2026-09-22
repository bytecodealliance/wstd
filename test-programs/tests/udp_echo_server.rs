use anyhow::{Context, Result};
use std::process::Command;

fn run(component: &str, p3: bool) -> Result<()> {
    use std::net::{SocketAddr, UdpSocket};
    use std::time::Duration;

    println!("testing {component}");

    // Run the component in wasmtime
    // -Sinherit-network required for sockets to work
    let mut command = Command::new("wasmtime");
    command.arg("run").arg("-Sinherit-network");
    if p3 {
        command.arg("-Sp3");
    }
    let mut wasmtime_process = command
        .arg(component)
        .stdout(std::process::Stdio::piped())
        .spawn()?;

    let addr = test_programs::get_listening_address(
        wasmtime_process.stdout.take().expect("stdout is piped"),
    )?;

    println!("udp echo server is listening on {addr:?}");

    // Connect each client so that it only receives its own echo, and give
    // every receive a deadline: a datagram may be dropped, and without a
    // timeout a lost echo would hang the test suite rather than fail it.
    fn client(addr: SocketAddr) -> Result<UdpSocket> {
        let sock = UdpSocket::bind("127.0.0.1:0").context("binding client socket")?;
        sock.set_read_timeout(Some(Duration::from_secs(10)))
            .context("setting client read timeout")?;
        sock.connect(addr).context("connecting client socket")?;
        Ok(sock)
    }

    let sock1 = client(addr).context("client sock1")?;
    println!("sock1 bound to {}", sock1.local_addr()?);

    let sock2 = client(addr).context("client sock2")?;
    println!("sock2 bound to {}", sock2.local_addr()?);

    const MESSAGE1: &[u8] = b"hello, echoserver!\n";
    // Exercise the datagram copy path with a larger second payload.
    const MESSAGE2: &[u8] = &[0xa5; 4096];

    sock1.send(MESSAGE1).context("send from sock1")?;
    println!("sock1 sent to echo server");

    sock2.send(MESSAGE2).context("send from sock2")?;
    println!("sock2 sent to echo server");

    let mut buf = vec![0; 65535];

    let len = sock1.recv(&mut buf).context("recv on sock1")?;
    println!("read from sock1");
    assert_eq!(MESSAGE1, &buf[..len], "readback of sock1");

    let len = sock2.recv(&mut buf).context("recv on sock2")?;
    println!("read from sock2");
    assert_eq!(MESSAGE2, &buf[..len], "readback of sock2");

    wasmtime_process.kill()?;

    Ok(())
}

#[test_log::test]
fn udp_echo_server_p2() -> Result<()> {
    run(test_programs::UDP_ECHO_SERVER, false)
}

#[test_log::test]
fn udp_echo_server_p3() -> Result<()> {
    if test_programs::NIGHTLY_TOOLCHAIN {
        run(test_programs::UDP_ECHO_SERVER_P3, true)?;
    }
    Ok(())
}
