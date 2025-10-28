use anyhow::Result;
use std::path::Path;
use std::process::Command;

fn run_s3_example() -> Command {
    let mut command = Command::new("wasmtime");
    command.arg("run");
    command.arg("-Shttp");
    command.args(["--env", "AWS_ACCESS_KEY_ID"]);
    command.args(["--env", "AWS_SECRET_ACCESS_KEY"]);
    command.args(["--env", "AWS_SESSION_TOKEN"]);
    command.args(["--dir", ".::."]);
    command.arg(test_programs::aws::S3);
    command
}

#[test_log::test]
fn aws_s3() -> Result<()> {
    // bucket list command
    let output = run_s3_example()
        .arg(format!(
            "--region={}",
            std::env::var("AWS_REGION").unwrap_or_else(|_| "us-west-2".to_owned())
        ))
        .arg(format!(
            "--bucket={}",
            std::env::var("WSTD_EXAMPLE_BUCKET")
                .unwrap_or_else(|_| "wstd-example-bucket".to_owned())
        ))
        .arg("list")
        .output()?;
    println!("{:?}", output);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("fluff.jpg"));
    assert!(stdout.contains("shoug.jpg"));

    // bucket get command
    let output = run_s3_example()
        .arg(format!(
            "--region={}",
            std::env::var("AWS_REGION").unwrap_or_else(|_| "us-west-2".to_owned())
        ))
        .arg(format!(
            "--bucket={}",
            std::env::var("WSTD_EXAMPLE_BUCKET")
                .unwrap_or_else(|_| "wstd-example-bucket".to_owned())
        ))
        .arg("get")
        .arg("shoug.jpg")
        .output()?;
    println!("{:?}", output);
    assert!(output.status.success());

    assert!(Path::new("shoug.jpg").exists());

    Ok(())
}
