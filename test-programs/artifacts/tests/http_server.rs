use anyhow::Result;
use std::net::TcpStream;
use std::process::{Child, Command};
use std::thread::sleep;
use std::time::{Duration, Instant};

// Wasmtime serve will run until killed. Kill it in a drop impl so the process
// isnt orphaned when the test suite ends (successfully, or unsuccessfully)
struct DontOrphan(Child);
impl Drop for DontOrphan {
    fn drop(&mut self) {
        let _ = self.0.kill();
    }
}

#[test_log::test]
fn http_server() -> Result<()> {
    // Run wasmtime serve.
    // Enable -Scli because we currently don't have a way to build with the
    // proxy adapter, so we build with the default adapter.
    let _wasmtime_process = DontOrphan(
        Command::new("wasmtime")
            .arg("serve")
            .arg("-Scli")
            .arg("--addr=127.0.0.1:8081")
            .arg(test_programs_artifacts::HTTP_SERVER)
            .spawn()?,
    );

    // Clumsily wait for the server to accept connections.
    'wait: loop {
        sleep(Duration::from_millis(100));
        if TcpStream::connect("127.0.0.1:8081").is_ok() {
            break 'wait;
        }
    }

    // Test each path in the server:

    // TEST / http_home
    // Response body is the hard-coded default
    let body: String = ureq::get("http://127.0.0.1:8081").call()?.into_string()?;
    assert_eq!(body, "Hello, wasi:http/proxy world!\n");

    // TEST /wait-response http_wait_response
    // Sleeps for 1 second, then sends a response with body containing
    // internally measured sleep time.
    let start = Instant::now();
    let body: String = ureq::get("http://127.0.0.1:8081/wait-response")
        .call()?
        .into_string()?;
    let duration = start.elapsed();
    let sleep_report = body
        .split(' ')
        .find_map(|s| s.parse::<usize>().ok())
        .expect("body should print 'slept for 10xx millis'");
    assert!(
        sleep_report >= 1000,
        "should have slept for 1000 or more millis, got {sleep_report}"
    );
    assert!(duration >= Duration::from_secs(1));

    // TEST /wait-body http_wait_body
    // Sends response status and headers, then sleeps for 1 second, then sends
    // body with internally measured sleep time.
    // With ureq we can't tell that the response status and headers were sent
    // with a delay in the body. Additionally, the implementation MAY buffer up the
    // entire response and body before sending it, though wasmtime does not.
    let start = Instant::now();
    let body: String = ureq::get("http://127.0.0.1:8081/wait-body")
        .call()?
        .into_string()?;
    let duration = start.elapsed();
    let sleep_report = body
        .split(' ')
        .find_map(|s| s.parse::<usize>().ok())
        .expect("body should print 'slept for 10xx millis'");
    assert!(
        sleep_report >= 1000,
        "should have slept for 1000 or more millis, got {sleep_report}"
    );
    assert!(duration >= Duration::from_secs(1));

    // TEST /stream-body http_stream_body
    // Sends response status and headers, then unfolds 5 iterations of a
    // stream that sleeps for 100ms and then prints the time since stream
    // started.
    // With ureq we can't tell that the response status and headers were sent
    // with a delay in the body. Additionally, the implementation MAY buffer up the
    // entire response and body before sending it, though wasmtime does not.
    let start = Instant::now();
    let body: String = ureq::get("http://127.0.0.1:8081/stream-body")
        .call()?
        .into_string()?;
    let duration = start.elapsed();
    assert_eq!(body.lines().count(), 5, "body has 5 lines");
    for (iter, line) in body.lines().enumerate() {
        let sleep_report = line
            .split(' ')
            .find_map(|s| s.parse::<usize>().ok())
            .expect("body should print 'stream started Nxx millis ago'");
        assert!(
            sleep_report >= (iter * 100),
            "should have slept for {iter} * 100 or more millis, got {sleep_report}"
        );
    }
    assert!(duration >= Duration::from_millis(500));

    // TEST /echo htto_echo
    // Send a request body, see that we got the same back in response body.
    const MESSAGE: &[u8] = b"hello, echoserver!\n";
    let body: String = ureq::get("http://127.0.0.1:8081/echo")
        .send(MESSAGE)?
        .into_string()?;
    assert_eq!(body.as_bytes(), MESSAGE);

    // TEST /echo-headers htto_echo_headers
    // Send request with headers, see that all of those headers are present in
    // response headers
    let test_headers = [
        ("Red", "Rhubarb"),
        ("Orange", "Carrots"),
        ("Yellow", "Bananas"),
        ("Green", "Broccoli"),
        ("Blue", "Blueberries"),
        ("Purple", "Beets"),
    ];
    let mut request = ureq::get("http://127.0.0.1:8081/echo-headers");
    for (name, value) in test_headers {
        request = request.set(name, value);
    }
    let response = request.call()?;
    assert!(response.headers_names().len() >= test_headers.len());
    for (name, value) in test_headers {
        assert_eq!(response.header(name), Some(value));
    }

    // NOT TESTED /echo-trailers htto_echo_trailers
    // ureq doesn't support trailers

    // TEST /response-code http_response_code
    // Send request with `X-Request-Code: <status>`. Should get back that
    // status.
    let response = ureq::get("http://127.0.0.1:8081/response-status")
        .set("X-Response-Status", "302")
        .call()?;
    assert_eq!(response.status(), 302);

    let response = ureq::get("http://127.0.0.1:8081/response-status")
        .set("X-Response-Status", "401")
        .call();
    // ureq interprets some statuses as OK, some as Err:
    match response {
        Err(ureq::Error::Status(401, _)) => {}
        result => {
            panic!("/response-code expected status 302, got: {result:?}");
        }
    }

    // TEST /response-fail http_response_fail
    // Wasmtime gives a 500 error when wasi-http guest gives error instead of
    // response
    match ureq::get("http://127.0.0.1:8081/response-fail").call() {
        Err(ureq::Error::Status(500, _)) => {}
        result => {
            panic!("/response-fail expected status 500 error, got: {result:?}");
        }
    }

    // TEST /response-body-fail http_body_fail
    // Response status and headers sent off, then error in body will close
    // connection
    match ureq::get("http://127.0.0.1:8081/response-body-fail").call() {
        Err(ureq::Error::Transport(_transport)) => {}
        result => {
            panic!("/response-body-fail expected transport error, got: {result:?}")
        }
    }

    Ok(())
}
