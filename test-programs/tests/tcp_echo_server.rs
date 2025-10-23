use anyhow::{Context, Result};
use std::process::Command;

#[test_log::test]
fn tcp_echo_server() -> Result<()> {
    use std::io::{Read, Write};
    use std::net::{Shutdown, TcpStream};

    println!("testing {}", test_programs::TCP_ECHO_SERVER);

    // Run the component in wasmtime
    // -Sinherit-network required for sockets to work
    let mut wasmtime_process = Command::new("wasmtime")
        .arg("run")
        .arg("-Sinherit-network")
        .arg(test_programs::TCP_ECHO_SERVER)
        .stdout(std::process::Stdio::piped())
        .spawn()?;

    let addr = get_listening_address(wasmtime_process.stdout.take().expect("stdout is piped"))?;

    println!("tcp echo server is listening on {addr:?}");

    let mut sock1 = TcpStream::connect(addr).context("connect sock1")?;
    println!("sock1 connected");

    let mut sock2 = TcpStream::connect(addr).context("connect sock2")?;
    println!("sock2 connected");

    const MESSAGE1: &[u8] = b"hello, echoserver!\n";

    sock1.write_all(MESSAGE1).context("write to sock1")?;
    println!("sock1 wrote to echo server");

    let mut sock3 = TcpStream::connect(addr).context("connect sock3")?;
    println!("sock3 connected");

    const MESSAGE2: &[u8] = b"hello, gussie!\n";
    sock2.write_all(MESSAGE2).context("write to sock1")?;
    println!("sock2 wrote to echo server");

    sock1.shutdown(Shutdown::Write)?;
    sock2.shutdown(Shutdown::Write)?;

    let mut readback2 = Vec::new();
    sock2
        .read_to_end(&mut readback2)
        .context("read from sock2")?;
    println!("read from sock2");

    let mut readback1 = Vec::new();
    sock1
        .read_to_end(&mut readback1)
        .context("read from sock1")?;
    println!("read from sock1");

    assert_eq!(MESSAGE1, readback1, "readback of sock1");
    assert_eq!(MESSAGE2, readback2, "readback of sock2");

    let mut sock4 = TcpStream::connect(addr).context("connect sock4")?;
    println!("sock4 connected");
    const MESSAGE4: &[u8] = b"hello, sparky!\n";
    sock4.write_all(MESSAGE4).context("write to sock4")?;
    // Hang up - demonstrate that a failure on this connection doesn't affect
    // others.
    drop(sock4);
    println!("sock4 hung up");

    const MESSAGE3: &[u8] = b"hello, willa!\n";
    sock3.write_all(MESSAGE3).context("write to sock3")?;
    println!("sock3 wrote to echo server");
    sock3.shutdown(Shutdown::Write)?;

    let mut readback3 = Vec::new();
    sock3
        .read_to_end(&mut readback3)
        .context("read from sock3")?;
    println!("read from sock3");
    assert_eq!(MESSAGE3, readback3, "readback of sock3");

    wasmtime_process.kill()?;

    Ok(())
}

fn get_listening_address(
    mut wasmtime_stdout: std::process::ChildStdout,
) -> Result<std::net::SocketAddr> {
    use std::io::Read;
    use std::thread::sleep;
    use std::time::Duration;

    // Gather complete contents of stdout here
    let mut stdout_contents = String::new();
    loop {
        // Wait for process to print
        sleep(Duration::from_millis(100));

        // Read more that the process printed, append to contents
        let mut buf = vec![0; 4096];
        let len = wasmtime_stdout
            .read(&mut buf)
            .context("reading wasmtime stdout")?;
        buf.truncate(len);
        stdout_contents
            .push_str(std::str::from_utf8(&buf).context("wasmtime stdout should be string")?);

        // Parse out the line where guest program says where it is listening
        for line in stdout_contents.lines() {
            if let Some(rest) = line.strip_prefix("Listening on ") {
                // Forget wasmtime_stdout, rather than drop it, so that any
                // subsequent stdout from wasmtime doesn't panic on a broken
                // pipe.
                std::mem::forget(wasmtime_stdout);
                return rest
                    .parse()
                    .with_context(|| format!("parsing socket addr from line: {line:?}"));
            }
        }
    }
}
