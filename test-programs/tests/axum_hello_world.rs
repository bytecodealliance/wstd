use anyhow::Result;
use std::net::TcpStream;
use std::process::{Child, Command};
use std::thread::sleep;
use std::time::Duration;

// Wasmtime serve will run until killed. Kill it in a drop impl so the process
// isnt orphaned when the test suite ends (successfully, or unsuccessfully)
struct DontOrphan(Child);
impl Drop for DontOrphan {
    fn drop(&mut self) {
        let _ = self.0.kill();
    }
}

#[test_log::test]
fn hello_world() -> Result<()> {
    run(test_programs::axum::HELLO_WORLD)
}

#[test_log::test]
fn hello_world_nomacro() -> Result<()> {
    run(test_programs::axum::HELLO_WORLD_NOMACRO)
}

// The hello_world.rs and hello_world_nomacro.rs are identical in
// functionality
fn run(guest: &str) -> Result<()> {
    // Run wasmtime serve.
    // Enable -Scli because we currently don't have a way to build with the
    // proxy adapter, so we build with the default adapter.
    let _wasmtime_process = DontOrphan(
        Command::new("wasmtime")
            .arg("serve")
            .arg("-Scli")
            .arg("--addr=127.0.0.1:8081")
            .arg(guest)
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

    // TEST / handler
    // Response body is the hard-coded default
    let body: String = ureq::get("http://127.0.0.1:8081").call()?.into_string()?;
    assert!(body.contains("<h1>Hello, World!</h1>"));

    Ok(())
}
