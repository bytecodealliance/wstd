//! Example s3 client running on `wstd` via `wstd_aws`
//!
//! This example is a wasi cli command. It accepts command line arguments
//! with the subcommand `list` to list a bucket's contents, and `get <key>`
//! to get an object from a bucket and write it to the filesystem.
//!
//! This example *must be compiled in release mode* - in debug mode, the aws
//! sdk's generated code will overflow the maximum permitted wasm locals in
//! a single function.
//!
//! Compile it with:
//!
//! ```sh
//! cargo build -p wstd-aws --target wasm32-wasip2 --release --examples
//! ```
//!
//! When running this example, you will need AWS credentials provided in environment
//! variables.
//!
//! Run it with:
//! ```sh
//! wasmtime run -Shttp \
//!     --env AWS_ACCESS_KEY_ID \
//!     --env AWS_SECRET_ACCESS_KEY \
//!     --env AWS_SESSION_TOKEN \
//!     --dir .::. \
//!     target/wasm22-wasip2/release/examples/s3.wasm
//! ```
//!
//! or alternatively run it with:
//! ```sh
//! cargo run --target wasm32-wasip2 -p wstd-aws --example s3
//! ```
//!
//! which uses the wasmtime cli, as above, via configiration found in this
//! workspace's `.cargo/config`.
//!
//! By default, this script accesses the `wstd-example-bucket` in `us-west-2`.
//! To change the bucket or region, use the `--bucket` and `--region` cli
//! flags before the subcommand.

use anyhow::Result;
use clap::{Parser, Subcommand};

use aws_config::{BehaviorVersion, Region};
use aws_sdk_s3::Client;

#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
struct Opts {
    /// The AWS Region. Defaults to us-west-2 if not provided.
    #[arg(short, long)]
    region: Option<String>,
    /// The name of the bucket. Defaults to wstd-example-bucket if not
    /// provided.
    #[arg(short, long)]
    bucket: Option<String>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    List,
    Get {
        key: String,
        #[arg(short, long)]
        out: Option<String>,
    },
}

#[wstd::main]
async fn main() -> Result<()> {
    let opts = Opts::parse();
    let region = opts
        .region
        .clone()
        .unwrap_or_else(|| "us-west-2".to_owned());
    let bucket = opts
        .bucket
        .clone()
        .unwrap_or_else(|| "wstd-example-bucket".to_owned());

    let config = aws_config::defaults(BehaviorVersion::latest())
        .region(Region::new(region))
        .sleep_impl(wstd_aws::sleep_impl())
        .http_client(wstd_aws::http_client())
        .load()
        .await;

    let client = Client::new(&config);

    match opts.command.as_ref().unwrap_or(&Command::List) {
        Command::List => {
            let output = list(&bucket, &client).await?;
            print!("{}", output);
        }
        Command::Get { key, out } => {
            let contents = get(&bucket, &client, key).await?;
            let output: &str = if let Some(out) = out {
                out.as_str()
            } else {
                key.as_str()
            };
            std::fs::write(output, contents)?;
        }
    }
    Ok(())
}

async fn list(bucket: &str, client: &Client) -> Result<String> {
    let mut listing = client
        .list_objects_v2()
        .bucket(bucket.to_owned())
        .into_paginator()
        .send();

    let mut output = String::new();
    output += "key\tetag\tlast_modified\tstorage_class\n";
    while let Some(res) = listing.next().await {
        let object = res?;
        for item in object.contents() {
            output += &format!(
                "{}\t{}\t{}\t{}\n",
                item.key().unwrap_or_default(),
                item.e_tag().unwrap_or_default(),
                item.last_modified()
                    .map(|lm| format!("{lm}"))
                    .unwrap_or_default(),
                item.storage_class()
                    .map(|sc| format!("{sc}"))
                    .unwrap_or_default(),
            );
        }
    }
    Ok(output)
}

async fn get(bucket: &str, client: &Client, key: &str) -> Result<Vec<u8>> {
    let object = client
        .get_object()
        .bucket(bucket.to_owned())
        .key(key)
        .send()
        .await?;
    let data = object.body.collect().await?;
    Ok(data.to_vec())
}
