include!(concat!(env!("OUT_DIR"), "/gen.rs"));

use std::fs::File;
use std::net::TcpStream;
use std::process::{Child, Command};
use std::thread::sleep;
use std::time::Duration;

/// Manages exclusive access to port 8081, and kills the process when dropped
pub struct WasmtimeServe {
    #[expect(dead_code, reason = "exists to live for as long as wasmtime process")]
    lockfile: File,
    process: Child,
}

impl WasmtimeServe {
    /// Run `wasmtime serve -Scli --addr=127.0.0.1:8081` for a given wasm
    /// guest filepath.
    ///
    /// Takes exclusive access to a lockfile so that only one test on a host
    /// can use port 8081 at a time.
    ///
    /// Kills the wasmtime process, and releases the lock, once dropped.
    pub fn new(guest: &str) -> std::io::Result<Self> {
        let mut lockfile = std::env::temp_dir();
        lockfile.push("TEST_PROGRAMS_WASMTIME_SERVE.lock");
        let lockfile = File::create(&lockfile)?;
        lockfile.lock()?;

        // Run wasmtime serve.
        // Enable -Scli because we currently don't have a way to build with the
        // proxy adapter, so we build with the default adapter.
        let process = Command::new("wasmtime")
            .arg("serve")
            .arg("-Scli")
            .arg("--addr=127.0.0.1:8081")
            .arg(guest)
            .spawn()?;
        let w = WasmtimeServe { lockfile, process };

        // Clumsily wait for the server to accept connections.
        'wait: loop {
            sleep(Duration::from_millis(100));
            if TcpStream::connect("127.0.0.1:8081").is_ok() {
                break 'wait;
            }
        }
        Ok(w)
    }
}
// Wasmtime serve will run until killed. Kill it in a drop impl so the process
// isnt orphaned when the test suite ends (successfully, or unsuccessfully)
impl Drop for WasmtimeServe {
    fn drop(&mut self) {
        let _ = self.process.kill();
    }
}
