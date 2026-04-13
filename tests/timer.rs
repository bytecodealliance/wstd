use std::error::Error;
use wstd::time::{Duration, Instant, Timer};

#[wstd::test]
async fn timer_after() -> Result<(), Box<dyn Error>> {
    let start = Instant::now();
    Timer::after(Duration::from_millis(50)).wait().await;
    let elapsed = start.elapsed();
    assert!(
        elapsed >= Duration::from_millis(40),
        "timer_after: elapsed {elapsed:?} should be >= 40ms"
    );
    Ok(())
}

#[wstd::test]
async fn timer_at() -> Result<(), Box<dyn Error>> {
    let start = Instant::now();
    let deadline = start + Duration::from_millis(50);
    Timer::at(deadline).wait().await;
    let elapsed = start.elapsed();
    assert!(
        elapsed >= Duration::from_millis(40),
        "timer_at: elapsed {elapsed:?} should be >= 40ms"
    );
    Ok(())
}

#[wstd::test]
async fn instant_monotonic() -> Result<(), Box<dyn Error>> {
    let a = Instant::now();
    let b = Instant::now();
    assert!(b >= a, "monotonic clock should not go backwards");
    Ok(())
}
