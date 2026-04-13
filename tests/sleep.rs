use std::error::Error;
use wstd::task::sleep;
use wstd::time::{Duration, Instant};

#[wstd::test]
async fn just_sleep() -> Result<(), Box<dyn Error>> {
    sleep(Duration::from_secs(1)).await;
    Ok(())
}

#[wstd::test]
async fn sleep_elapsed() -> Result<(), Box<dyn Error>> {
    let start = Instant::now();
    sleep(Duration::from_millis(100)).await;
    let elapsed = start.elapsed();
    assert!(
        elapsed >= Duration::from_millis(80),
        "sleep: elapsed {elapsed:?} should be >= 80ms"
    );
    assert!(
        elapsed < Duration::from_secs(2),
        "sleep: elapsed {elapsed:?} should be < 2s"
    );
    Ok(())
}
