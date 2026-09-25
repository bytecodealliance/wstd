use anyhow::{Context, Result};
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Command, Stdio};

fn run(component: &str, p3: bool) -> Result<()> {
    let mut command = Command::new("wasmtime");
    command.arg("run");
    if p3 {
        command.arg("-Sp3");
    }

    let mut child = command
        .arg(component)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let mut stdin = child.stdin.take().context("child stdin")?;
    let mut stdout = BufReader::new(child.stdout.take().context("child stdout")?);
    let mut stderr = BufReader::new(child.stderr.take().context("child stderr")?);

    stdin.write_all(b"hello from stdin\n")?;
    stdin.flush()?;

    let mut line = String::new();
    stdout.read_line(&mut line)?;
    assert_eq!(line, "stdout: hello from stdin\n");

    line.clear();
    stderr.read_line(&mut line)?;
    assert_eq!(line, "stderr: hello from stdin\n");

    // Closing stdin ends the guest's read loop. Closing stdout makes its next
    // write fail, which the guest reports through the still-open stderr pipe.
    drop(stdin);
    drop(stdout);

    let mut errors = String::new();
    stderr.read_to_string(&mut errors)?;

    let status = child.wait()?;
    assert!(status.success(), "stdio example failed: {status}");
    assert_eq!(
        errors,
        format!("stdin error: UnexpectedEof\nstdout error: ConnectionReset\n")
    );

    Ok(())
}

#[test_log::test]
fn stdio_p2() -> Result<()> {
    run(test_programs::STDIO, false)
}

#[test_log::test]
fn stdio_p3() -> Result<()> {
    // TODO: Remove this nightly check once wasm32-wasip3 is available on stable.
    if test_programs::NIGHTLY_TOOLCHAIN {
        run(test_programs::STDIO_P3, true)?;
    }

    Ok(())
}
