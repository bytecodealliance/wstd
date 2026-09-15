use std::cell::Cell;
use std::future::pending;
use std::rc::Rc;

use futures_lite::future::yield_now;

struct DropFlag(Rc<Cell<bool>>);

impl Drop for DropFlag {
    fn drop(&mut self) {
        self.0.set(true);
    }
}

async fn wait_until(flag: &Cell<bool>) {
    while !flag.get() {
        yield_now().await;
    }
}

#[wstd::test]
async fn spawn_detach_cancel_and_drop() {
    assert_eq!(wstd::runtime::spawn(async { 42 }).await, 42);

    // Check that a detached task completes eventually.
    let detached_completed = Rc::new(Cell::new(false));
    let completed = detached_completed.clone();
    wstd::runtime::spawn(async move {
        yield_now().await;
        completed.set(true);
    })
    .detach();

    assert!(!detached_completed.get());
    wait_until(&detached_completed).await;
    assert!(detached_completed.get());

    // Check that calling `cancel` on a task cancels and drops the spawned
    // future.
    let canceled_started = Rc::new(Cell::new(false));
    let canceled_dropped = Rc::new(Cell::new(false));
    let canceled_completed = Rc::new(Cell::new(false));
    let started = canceled_started.clone();
    let dropped = canceled_dropped.clone();
    let completed = canceled_completed.clone();
    let task = wstd::runtime::spawn(async move {
        let _drop_flag = DropFlag(dropped);
        started.set(true);
        pending::<()>().await;
        completed.set(true);
    });

    wait_until(&canceled_started).await;
    assert_eq!(task.cancel().await, None);
    assert!(canceled_dropped.get());
    assert!(!canceled_completed.get());

    // Check that dropping a task cancels and drops the spawned future.
    let dropped_started = Rc::new(Cell::new(false));
    let dropped_dropped = Rc::new(Cell::new(false));
    let dropped_completed = Rc::new(Cell::new(false));
    let started = dropped_started.clone();
    let dropped = dropped_dropped.clone();
    let completed = dropped_completed.clone();
    let task = wstd::runtime::spawn(async move {
        let _drop_flag = DropFlag(dropped);
        started.set(true);
        pending::<()>().await;
        completed.set(true);
    });

    wait_until(&dropped_started).await;
    drop(task);
    wait_until(&dropped_dropped).await;
    assert!(!dropped_completed.get());
}
