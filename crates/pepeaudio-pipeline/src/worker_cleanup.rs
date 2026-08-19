use std::time::Duration;

use tokio::{task::JoinHandle, time::timeout};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorkerCleanup {
    Finished,
    Failed,
    AbortedAfterTimeout,
}

pub(crate) async fn finish_worker(mut worker: JoinHandle<()>, wait: Duration) -> WorkerCleanup {
    match timeout(wait, &mut worker).await {
        Ok(Ok(())) => WorkerCleanup::Finished,
        Ok(Err(_)) => WorkerCleanup::Failed,
        Err(_) => {
            worker.abort();
            let _ = worker.await;
            WorkerCleanup::AbortedAfterTimeout
        }
    }
}

#[cfg(test)]
mod tests;
