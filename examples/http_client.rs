use anyhow::{anyhow, Result};
use clap::{ArgAction, Parser};
use wstd::http::{Body, BodyExt, Client, Method, Request, Uri};
use wstd::io::AsyncWrite;

/// Simple HTTP client
///
/// A simple command-line HTTP client, implemented using `wstd`, using WASI.
#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    /// The URL to request
    url: Uri,

    /// Forward stdin to the request body
    #[arg(long)]
    body: bool,

    /// Add a header to the request
    #[arg(long = "header", action = ArgAction::Append, value_name = "HEADER")]
    headers: Vec<String>,

    /// Method of the request
    #[arg(long, default_value = "GET")]
    method: Method,

    /// Set the connect timeout
    #[arg(long, value_name = "DURATION")]
    connect_timeout: Option<humantime::Duration>,

    /// Set the first-byte timeout
    #[arg(long, value_name = "DURATION")]
    first_byte_timeout: Option<humantime::Duration>,

    /// Set the between-bytes timeout
    #[arg(long, value_name = "DURATION")]
    between_bytes_timeout: Option<humantime::Duration>,
}

#[wstd::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Create and configure the `Client`

    let mut client = Client::new();

    if let Some(connect_timeout) = args.connect_timeout {
        client.set_connect_timeout(*connect_timeout);
    }
    if let Some(first_byte_timeout) = args.first_byte_timeout {
        client.set_first_byte_timeout(*first_byte_timeout);
    }
    if let Some(between_bytes_timeout) = args.between_bytes_timeout {
        client.set_between_bytes_timeout(*between_bytes_timeout);
    }

    // Create and configure the request.

    let mut request = Request::builder();

    request = request.uri(args.url).method(args.method);

    for header in args.headers {
        let mut parts = header.splitn(2, ": ");
        let key = parts.next().unwrap();
        let value = parts
            .next()
            .ok_or_else(|| anyhow!("headers must be formatted like \"key: value\""))?;
        request = request.header(key, value);
    }

    // Send the request.

    let body = if args.body {
        Body::from_try_stream(wstd::io::stdin().into_inner().into_stream())
    } else {
        Body::empty()
    };

    let request = request.body(body)?;

    eprintln!("> {} / {:?}", request.method(), request.version());
    for (key, value) in request.headers().iter() {
        let value = String::from_utf8_lossy(value.as_bytes());
        eprintln!("> {key}: {value}");
    }

    let response = client.send(request).await?;

    // Print the response.
    eprintln!("< {:?} {}", response.version(), response.status());
    for (key, value) in response.headers().iter() {
        let value = String::from_utf8_lossy(value.as_bytes());
        eprintln!("< {key}: {value}");
    }

    let body = response.into_body().into_boxed_body().collect().await?;
    let trailers = body.trailers().cloned();
    wstd::io::stdout()
        .write_all(body.to_bytes().as_ref())
        .await?;

    if let Some(trailers) = trailers {
        for (key, value) in trailers.iter() {
            let value = String::from_utf8_lossy(value.as_bytes());
            eprintln!("< {key}: {value}");
        }
    }

    Ok(())
}
