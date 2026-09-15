// Verify that the test macro is acutally running.

#[wstd::test]
async fn computation() -> Result<(), String> {
    assert_eq!(1 + 1, 2);
    Ok(())
}

#[wstd::test]
async fn second_computation() {
    assert_eq!(1 + 1, 2);
}
