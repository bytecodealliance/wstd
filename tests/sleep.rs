use std::cell::RefCell;
use std::error::Error;
use std::rc::Rc;
use wstd::task::sleep;
use wstd::time::Duration;

#[wstd::test]
async fn just_sleep() -> Result<(), Box<dyn Error>> {
    sleep(Duration::from_secs(1)).await;
    Ok(())
}

#[wstd::test]
async fn concurrent_sleeps_wake_in_deadline_order() {
    let wake_order = Rc::new(RefCell::new(Vec::new()));
    let mut tasks = Vec::new();

    for (delay, task) in [(60, "slow"), (20, "fast"), (40, "medium")] {
        let wake_order = wake_order.clone();
        tasks.push(wstd::runtime::spawn(async move {
            sleep(Duration::from_millis(delay)).await;
            wake_order.borrow_mut().push(task);
        }));
    }

    for task in tasks {
        task.await;
    }

    assert_eq!(*wake_order.borrow(), ["fast", "medium", "slow"]);
}
