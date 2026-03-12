
# wstd-aws-example: using wstd support in the AWS Rust SDK

The AWS Rust SDK has support for using the [`wstd`] crate on the
`wasm32-wasip2` target to use the wasi-http interface. This example shows how
to use it.

## TL;DR 

* depend on `aws-*` crates released recently enough to have MSRV of 1.91.1 (on
  or after March 4, 2026). Use `default-features = false` so tokio doesn't get
  sucked in.
* depend on `aws-smithy-wasm` and setup your `Config` with:
    ```
    config
        .sleep_impl(aws_smithy_wasm::wasi::WasiSleep)
        .http_client(aws_smithy_wasm::wasi::WasiHttpClientBuilder::new().build())
    ```

## Explanation

In many wasi settings, its necessary or desirable to use the wasi-http
interface to make http requests. Wasi-http interfaces provide an http
implementation, including the sockets layer and TLS, outside of the user's
component. `wstd` provides user-friendly async Rust interfaces to all of the
standardized wasi interfaces, including wasi-http.

The AWS Rust SDK, by default, depends on `tokio`, `hyper`, and either `rustls`
or `s2n_tls`, and makes http requests over sockets (which can be provided as
wasi-sockets). Those dependencies may not work correctly under `wasm32-wasip2`,
and if they do, they will not use the wasi-http interfaces. To avoid using
http over sockets, make sure to set the `default-features = false` setting
when depending on any `aws-*` crates in your project.

To configure the AWS Rust SDK to use `wstd`'s wasi-http client, use the
[`aws_smithy_crate`](https://docs.rs/aws-smithy-wasm/latest/aws_smithy_wasm/)
at version 0.10.0 or later. Provide `aws_smithy_wasm::wasi::WasiSleep` and
`aws_smithy_wasm::wasi::WasiHttpClientBuilder::new().build()` to your
[`aws_config::ConfigLoader`]:

```
    let config = aws_config::defaults(BehaviorVersion::latest())
        .sleep_impl(aws_smithy_wasm::wasi::WasiSleep)
        .http_client(aws_smithy_wasm::wasi::WasiHttpClientBuilder::new().build())
        ...;
```

[`wstd`]: https://docs.rs/wstd/latest/wstd
[`aws_config::ConfigLoader`]: https://docs.rs/aws-config/1.8.8/aws_config/struct.ConfigLoader.html

## Example

An example s3 client is provided as a wasi cli command. It accepts command
line arguments with the subcommand `list` to list a bucket's contents, and
`get <key>` to get an object from a bucket and write it to the filesystem.

This example *must be compiled in release mode* - in debug mode, the aws
sdk's generated code will overflow the maximum permitted wasm locals in
a single function.

Compile it with:

```sh
cargo build -p wstd-aws-example --target wasm32-wasip2 --release
```

When running this example, you will need AWS credentials provided in environment
variables, and you should substitute in a region and bucket where your
credentials have permissions to list the bucket and read items.

Run it with:
```sh
wasmtime run -Shttp \
    --env AWS_ACCESS_KEY_ID \
    --env AWS_SECRET_ACCESS_KEY \
    --env AWS_SESSION_TOKEN \
    --dir .::. \
    target/wasm32-wasip2/release/s3.wasm \
    --region us-west-2 \
    --bucket wstd-example-bucket
```

or alternatively run it with:
```sh
cargo run --target wasm32-wasip2 -p wstd-aws-example --example s3 -- \
    --region us-west-2 --bucket wstd-example-bucket
```
which uses the wasmtime cli, as above, via configiration found in this
workspace's `.cargo/config.toml`.

By default, the subcommand `list` will be run, listing the contents of the
bucket. To get an item from the bucket, use the subcommand `get <key> [-o
<output>]`. Use `--help` when in doubt.

