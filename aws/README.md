
# wstd-aws: wstd support for the AWS Rust SDK

This crate provides support for using the AWS Rust SDK for the `wasm32-wasip2`
target using the [`wstd`] crate.

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

To configure `wstd`'s wasi-http client for the AWS Rust SDK, provide
`wstd_aws::sleep_impl()` and `wstd_aws::http_client()` to your
[`aws_config::ConfigLoader`]:

```
    let config = aws_config::defaults(BehaviorVersion::latest())
        .sleep_impl(wstd_aws::sleep_impl())
        .http_client(wstd_aws::http_client())
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
cargo build -p wstd-aws --target wasm32-wasip2 --release --examples
```

When running this example, you will need AWS credentials provided in environment
variables.

Run it with:
```sh
wasmtime run -Shttp \
    --env AWS_ACCESS_KEY_ID \
    --env AWS_SECRET_ACCESS_KEY \
    --env AWS_SESSION_TOKEN \
    --dir .::. \
    target/wasm32-wasip2/release/examples/s3.wasm
```

or alternatively run it with:
```sh
cargo run --target wasm32-wasip2 -p wstd-aws --example s3
```

which uses the wasmtime cli, as above, via configiration found in this
workspace's `.cargo/config`.

By default, this script accesses the `wstd-example-bucket` in `us-west-2`.
To change the bucket or region, use the `--bucket` and `--region` cli
flags before the subcommand.


