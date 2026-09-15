#![cfg(target_os = "wasi")]

//! Verifies that the `main` macro is compiling.

#[wstd::main]
async fn main() {
    println!("Hello world");
}
