use std::{sync::Arc, time::Duration};

use pepeaudio_media::{JanitorError, ManagedDownloadJanitor};
use tokio::{sync::watch, task::JoinHandle};

use crate::BotError;

pub(crate) struct MediaJanitorRuntime {
    shutdown: watch::Sender<bool>,
    task: Option<JoinHandle<Result<(), JanitorError>>>,
}

impl MediaJanitorRuntime {
    pub(crate) fn start(janitor: Arc<ManagedDownloadJanitor>, interval: Duration) -> Self {
        let (shutdown, receiver) = watch::channel(false);
        let task = tokio::spawn(async move {
            janitor
                .run_periodic(interval, receiver, |report| {
                    if !report.removals.is_empty() || !report.skipped.is_empty() {
                        tracing::info!(
                            scanned = report.scanned_entries,
                            removed = report.removals.len(),
                            skipped = report.skipped.len(),
                            truncated = report.scan_limit_reached,
                            "managed media cleanup completed"
                        );
                    }
                })
                .await
        });
        Self {
            shutdown,
            task: Some(task),
        }
    }

    pub(crate) async fn wait_for_unexpected_exit(&mut self) -> BotError {
        let outcome = match self.task.as_mut() {
            Some(task) => task.await,
            None => return BotError::MediaJanitorStopped,
        };
        self.task.take();
        match outcome {
            Ok(Ok(())) => BotError::MediaJanitorStopped,
            Ok(Err(error)) => BotError::MediaJanitor(error),
            Err(error) => BotError::MediaJanitorTask(error),
        }
    }

    pub(crate) async fn shutdown(mut self) -> bool {
        let Some(task) = self.task.take() else {
            return true;
        };
        let signalled = self.shutdown.send(true).is_ok();
        let joined = task.await.is_ok_and(|result| result.is_ok());
        signalled && joined
    }

    #[cfg(test)]
    fn from_task(
        shutdown: watch::Sender<bool>,
        task: JoinHandle<Result<(), JanitorError>>,
    ) -> Self {
        Self {
            shutdown,
            task: Some(task),
        }
    }
}

impl Drop for MediaJanitorRuntime {
    fn drop(&mut self) {
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use pepeaudio_media::JanitorError;
    use tokio::sync::watch;

    use super::MediaJanitorRuntime;
    use crate::BotError;

    #[tokio::test]
    async fn clean_return_is_unexpected_without_shutdown() {
        let (shutdown, _receiver) = watch::channel(false);
        let task = tokio::spawn(async { Ok(()) });
        let mut runtime = MediaJanitorRuntime::from_task(shutdown, task);

        assert!(matches!(
            runtime.wait_for_unexpected_exit().await,
            BotError::MediaJanitorStopped
        ));
        assert!(runtime.shutdown().await);
    }

    #[tokio::test]
    async fn janitor_error_remains_the_source() {
        let (shutdown, _receiver) = watch::channel(false);
        let task = tokio::spawn(async { Err(JanitorError::InvalidInterval) });
        let mut runtime = MediaJanitorRuntime::from_task(shutdown, task);

        assert!(matches!(
            runtime.wait_for_unexpected_exit().await,
            BotError::MediaJanitor(JanitorError::InvalidInterval)
        ));
    }

    #[tokio::test]
    async fn panic_is_reported_as_a_task_failure() {
        async fn panic_in_task() -> Result<(), JanitorError> {
            tokio::task::yield_now().await;
            panic!("janitor task panic");
        }

        let (shutdown, _receiver) = watch::channel(false);
        let task = tokio::spawn(panic_in_task());
        let mut runtime = MediaJanitorRuntime::from_task(shutdown, task);

        match runtime.wait_for_unexpected_exit().await {
            BotError::MediaJanitorTask(error) => assert!(error.is_panic()),
            error => panic!("unexpected error: {error}"),
        }
    }

    #[tokio::test]
    async fn coordinated_shutdown_signals_and_joins() {
        let (shutdown, mut receiver) = watch::channel(false);
        let task = tokio::spawn(async move {
            receiver.changed().await.expect("sender remains alive");
            assert!(*receiver.borrow());
            Ok(())
        });
        let runtime = MediaJanitorRuntime::from_task(shutdown, task);

        assert!(runtime.shutdown().await);
    }
}
