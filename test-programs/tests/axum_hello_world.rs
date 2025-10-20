use anyhow::Result;

#[test_log::test]
fn hello_world() -> Result<()> {
    run(test_programs::axum::HELLO_WORLD)
}

#[test_log::test]
fn hello_world_nomacro() -> Result<()> {
    run(test_programs::axum::HELLO_WORLD_NOMACRO)
}

// The hello_world.rs and hello_world_nomacro.rs are identical in
// functionality
fn run(guest: &str) -> Result<()> {
    // Run wasmtime serve.
    let _serve = test_programs::WasmtimeServe::new(guest)?;

    // Test each path in the server:

    // TEST / handler
    // Response body is the hard-coded default
    let body: String = ureq::get("http://127.0.0.1:8081")
        .call()?
        .body_mut()
        .read_to_string()?;
    assert!(body.contains("<h1>Hello, World!</h1>"));

    Ok(())
}
