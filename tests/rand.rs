/// Check that the rand interface works with empty and non-empty buffers.
#[wstd::test]
async fn fills_random_bytes() {
    let mut secure = [0; 32];
    wstd::rand::get_random_bytes(&mut secure);
    assert!(secure.iter().any(|byte| *byte != 0));

    let mut insecure = [0; 32];
    wstd::rand::get_insecure_random_bytes(&mut insecure);
    assert!(insecure.iter().any(|byte| *byte != 0));

    wstd::rand::get_random_bytes(&mut []);
    wstd::rand::get_insecure_random_bytes(&mut []);
}
