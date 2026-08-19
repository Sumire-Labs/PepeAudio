use std::{
    future::pending,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use super::{WorkerCleanup, finish_worker};

#[tokio::test]
async fn completed_worker_finishes_cleanly() {
    let worker = tokio::spawn(async {});

    assert_eq!(
        finish_worker(worker, Duration::from_secs(1)).await,
        WorkerCleanup::Finished
    );
}

#[tokio::test]
async fn panicked_worker_is_reported_after_it_is_reaped() {
    let worker = tokio::spawn(async { panic!("test worker failure") });

    assert_eq!(
        finish_worker(worker, Duration::from_secs(1)).await,
        WorkerCleanup::Failed
    );
}

#[tokio::test]
async fn stalled_worker_is_aborted_at_the_deadline() {
    let dropped = Arc::new(AtomicBool::new(false));
    let guard = DropFlag(Arc::clone(&dropped));
    let worker = tokio::spawn(async move {
        let _guard = guard;
        pending::<()>().await;
    });
    tokio::task::yield_now().await;

    assert_eq!(
        finish_worker(worker, Duration::from_millis(10)).await,
        WorkerCleanup::AbortedAfterTimeout
    );
    assert!(dropped.load(Ordering::Acquire));
}

struct DropFlag(Arc<AtomicBool>);

impl Drop for DropFlag {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}
