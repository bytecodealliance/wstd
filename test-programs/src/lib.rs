include!(concat!(env!("OUT_DIR"), "/gen.rs"));

use anyhow::{Context, Result, bail};
use std::fs::File;
use std::net::TcpStream;
use std::process::{Child, Command};
use std::thread::sleep;
use std::time::Duration;

// Required until msrv over 1.89, at which point locking is available in std
use fs2::FileExt;

/// Manages exclusive access to port 8081, and kills the process when dropped
pub struct WasmtimeServe {
    process: Child,
    addr: Option<std::net::SocketAddr>,
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
        Self::new_with_config(guest, &[])
    }

    pub fn new_with_config(guest: &str, env_vars: &[&str]) -> std::io::Result<Self> {
        // Run wasmtime serve.
        // Enable -Scli because we currently don't have a way to build with the
        // proxy adapter, so we build with the default adapter.
        let mut process = Command::new("wasmtime");
        let listening_addr = format!("127.0.0.1:0");
        process
            .arg("serve")
            .arg("-Scli")
            .arg("--addr")
            .arg(&listening_addr);
        for env_var in env_vars {
            process.arg("--env").arg(env_var);
        }
        let process = process
            .arg(guest)
            .stderr(std::process::Stdio::piped())
            .spawn()?;
        let mut w = WasmtimeServe {
            process,
            addr: None,
        };
        let listening_addr = get_listening_address(w.process.stderr.as_mut().unwrap())
            .expect("failed to get listening address");
        w.addr = Some(listening_addr);
        // Clumsily wait for the server to accept connections.
        'wait: loop {
            sleep(Duration::from_millis(100));
            if TcpStream::connect(listening_addr).is_ok() {
                break 'wait;
            }
        }
        Ok(w)
    }

    pub fn get_listening_address(&self) -> std::net::SocketAddr {
        self.addr.unwrap()
    }
}
// Wasmtime serve will run until killed. Kill it in a drop impl so the process
// isnt orphaned when the test suite ends (successfully, or unsuccessfully)
impl Drop for WasmtimeServe {
    fn drop(&mut self) {
        let _ = self.process.kill();
    }
}

/// Read a guest's stdout until it reports where it is listening, and return
/// that address.
///
/// Guest programs which bind a socket print `Listening on {addr}`, so that a
/// test can discover the address even when the guest picked the port.
pub fn get_listening_address(
    wasmtime_stdout: &mut impl std::io::Read,
) -> Result<std::net::SocketAddr> {
    let mut stdout_contents = String::new();
    let mut buf = [0; 4096];
    loop {
        let len = wasmtime_stdout
            .read(&mut buf)
            .context("reading wasmtime stdout")?;
        if len == 0 {
            bail!("wasmtime exited before reporting its listening address");
        }
        stdout_contents.push_str(
            std::str::from_utf8(&buf[..len]).context("wasmtime stdout should be string")?,
        );

        // Parse out the line where guest program says where it is listening
        for line in stdout_contents.lines() {
            if let Some(rest) = line.strip_prefix("Listening on ") {
                return rest
                    .parse()
                    .with_context(|| format!("parsing socket addr from line: {line:?}"));
            } else if let Some(rest) = line.strip_prefix("Serving HTTP on http://") {
                return rest
                    .trim_end_matches("/")
                    .parse()
                    .with_context(|| format!("parsing http addr from line {line:?}"));
            }
        }
    }
}
