use anyhow::Result;
use clap::{Parser, Subcommand};

use aws_config::{BehaviorVersion, Region};
use aws_sdk_s3::Client;

#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
struct Opts {
    /// The AWS Region.
    #[arg(short, long)]
    region: String,
    /// The name of the bucket.
    #[arg(short, long)]
    bucket: String,

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
    let config = aws_config::defaults(BehaviorVersion::latest())
        .region(Region::new(opts.region.clone()))
        .sleep_impl(wstd_aws::sleep_impl())
        .http_client(wstd_aws::http_client())
        .load()
        .await;

    let client = Client::new(&config);

    match opts.command.as_ref().unwrap_or(&Command::List) {
        Command::List => {
            let output = list(&opts, &client).await?;
            print!("{}", output);
        }
        Command::Get { key, out } => {
            let contents = get(&opts, &client, key).await?;
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

async fn list(opts: &Opts, client: &Client) -> Result<String> {
    let mut listing = client
        .list_objects_v2()
        .bucket(opts.bucket.clone())
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

async fn get(opts: &Opts, client: &Client, key: &str) -> Result<Vec<u8>> {
    let object = client
        .get_object()
        .bucket(opts.bucket.clone())
        .key(key)
        .send()
        .await?;
    let data = object.body.collect().await?;
    Ok(data.to_vec())
}
