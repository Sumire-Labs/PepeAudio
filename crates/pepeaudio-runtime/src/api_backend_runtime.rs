use tokio::{sync::watch, task::JoinHandle};

use crate::{RuntimeError, RuntimeResult};

pub struct ApiBackendRuntime {
    pub(super) shutdown: Option<watch::Sender<bool>>,
    pub(super) task: Option<JoinHandle<()>>,
}

impl ApiBackendRuntime {
    /// Waits for an uncoordinated subscription-task exit. Cancellation is safe;
    /// the runtime retains the join handle for a later shutdown.
    pub async fn wait_for_unexpected_exit(&mut self) -> RuntimeError {
        let outcome = match self.task.as_mut() {
            Some(task) => task.await,
            None => {
                return RuntimeError::RequiredTaskStopped {
                    task: "api snapshot subscription",
                };
            }
        };
        self.task.take();
        match outcome {
            Ok(()) => RuntimeError::RequiredTaskStopped {
                task: "api snapshot subscription",
            },
            Err(error) => RuntimeError::Task(error),
        }
    }

    /// # Errors
    ///
    /// Returns if the subscription task panics or is cancelled.
    pub async fn shutdown(mut self) -> RuntimeResult<()> {
        if let Some(shutdown) = self.shutdown.take() {
            let _receivers = shutdown.send(true);
        }
        if let Some(task) = self.task.take() {
            task.await.map_err(RuntimeError::Task)?;
        }
        Ok(())
    }
}

impl Drop for ApiBackendRuntime {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _receivers = shutdown.send(true);
        }
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}
