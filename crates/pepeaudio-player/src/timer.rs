use std::time::Duration;

use tokio::{sync::mpsc, task::JoinHandle};

use crate::{IdleGeneration, PlayerError};

#[derive(Clone, Copy, Debug)]
pub(crate) struct IdleExpired {
    pub(crate) generation: IdleGeneration,
}

pub(crate) struct IdleTimer {
    generation: IdleGeneration,
    task: Option<JoinHandle<()>>,
    active: bool,
}

impl IdleTimer {
    pub(crate) fn new() -> Self {
        Self {
            generation: IdleGeneration::default(),
            task: None,
            active: false,
        }
    }

    pub(crate) fn generation(&self) -> IdleGeneration {
        self.generation
    }

    pub(crate) fn is_active(&self) -> bool {
        self.active
    }

    pub(crate) fn arm(
        &mut self,
        timeout: Duration,
        sender: mpsc::Sender<IdleExpired>,
    ) -> Result<IdleGeneration, PlayerError> {
        self.invalidate()?;
        let generation = self.generation;
        self.active = true;
        let deadline = tokio::time::Instant::now() + timeout;
        self.task = Some(tokio::spawn(async move {
            tokio::time::sleep_until(deadline).await;
            let _ = sender.send(IdleExpired { generation }).await;
        }));
        Ok(generation)
    }

    pub(crate) fn cancel(&mut self) -> Result<IdleGeneration, PlayerError> {
        self.invalidate()?;
        Ok(self.generation)
    }

    pub(crate) fn mark_fired(&mut self, generation: IdleGeneration) {
        if generation == self.generation {
            self.task = None;
            self.active = false;
        }
    }

    fn invalidate(&mut self) -> Result<(), PlayerError> {
        if let Some(task) = self.task.take() {
            task.abort();
        }
        self.active = false;
        self.generation = self
            .generation
            .next()
            .ok_or(PlayerError::IdleGenerationExhausted)?;
        Ok(())
    }
}

impl Drop for IdleTimer {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::sync::mpsc;

    use super::IdleTimer;

    #[tokio::test(start_paused = true)]
    async fn cancelling_invalidates_an_expiration_already_in_the_mailbox() {
        let (sender, mut receiver) = mpsc::channel(1);
        let mut timer = IdleTimer::new();
        let armed = timer
            .arm(Duration::from_mins(5), sender)
            .expect("timer arms");

        tokio::time::advance(Duration::from_mins(5)).await;
        tokio::task::yield_now().await;
        let current = timer.cancel().expect("timer cancels");
        let expiration = receiver.recv().await.expect("expiration was queued");

        assert_eq!(expiration.generation, armed);
        assert_ne!(expiration.generation, current);
    }
}
