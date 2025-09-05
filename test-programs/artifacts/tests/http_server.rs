use anyhow::Result;
use std::process::Command;

#[test_log::test]
fn http_server() -> Result<()> {
    use std::net::TcpStream;
    use std::thread::sleep;
    use std::time::Duration;

    // Run wasmtime serve.
    // Enable -Scli because we currently don't have a way to build with the
    // proxy adapter, so we build with the default adapter.
    let mut wasmtime_process = Command::new("wasmtime")
        .arg("serve")
        .arg("-Scli")
        .arg("--addr=127.0.0.1:8081")
        .arg(test_programs_artifacts::HTTP_SERVER)
        .spawn()?;

    // Clumsily wait for the server to accept connections.
    'wait: loop {
        sleep(Duration::from_millis(100));
        if TcpStream::connect("127.0.0.1:8081").is_ok() {
            break 'wait;
        }
    }

    // Do some tests!

    let body: String = ureq::get("http://127.0.0.1:8081").call()?.into_string()?;
    assert_eq!(body, "Hello, wasi:http/proxy world!\n");

    match ureq::get("http://127.0.0.1:8081/fail").call() {
        Ok(body) => {
            panic!("unexpected success from /fail: {:?}", body);
        }
        Err(ureq::Error::Transport(_transport)) => {}
        Err(other) => {
            panic!("unexpected error: {:?}", other);
        }
    }

    const MESSAGE: &[u8] = b"hello, echoserver!\n";

    let body: String = ureq::get("http://127.0.0.1:8081/echo")
        .send(MESSAGE)?
        .into_string()?;
    assert_eq!(body.as_bytes(), MESSAGE);

    let test_headers = [
        ("Red", "Rhubarb"),
        ("Orange", "Carrots"),
        ("Yellow", "Bananas"),
        ("Green", "Broccoli"),
        ("Blue", "Blueberries"),
        ("Purple", "Beets"),
    ];

    let mut response = ureq::get("http://127.0.0.1:8081/echo-headers");
    for (name, value) in test_headers {
        response = response.set(name, value);
    }
    let response = response.call()?;

    assert!(response.headers_names().len() >= test_headers.len());
    for (name, value) in test_headers {
        assert_eq!(response.header(name), Some(value));
    }

    wasmtime_process.kill()?;

    Ok(())
}
