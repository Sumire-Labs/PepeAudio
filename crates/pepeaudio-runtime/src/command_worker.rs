use std::{ops::Range, sync::Arc, time::Duration};

use async_trait::async_trait;
use pepeaudio_core::GuildId;
use pepeaudio_player::PlayerHandle;
use pepeaudio_storage::{CommandConsumer, CommandResultStore, IdempotencyStore};
use thiserror::Error;
use tokio::{sync::watch, task::JoinSet};

use crate::{CommandAuthorizer, RuntimeError, RuntimeResult, command_dispatch::run_shard};

pub const DEFAULT_COMMAND_RESULT_RETENTION: Duration = Duration::from_hours(24);

/// Resolves only players owned by this Discord shard process.
#[async_trait]
pub trait PlayerDirectory: Send + Sync + 'static {
    async fn player(&self, guild_id: GuildId) -> Result<Option<PlayerHandle>, WorkerPlayerError>;
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum WorkerPlayerError {
    #[error("local player registry unavailable")]
    Unavailable,
}

#[derive(Clone, Debug)]
pub struct CommandWorkerConfig {
    /// Half-open shard range owned by this process.
    pub shards: Range<u32>,
    /// Stable Valkey consumer group shared by all Bot instances.
    pub group: String,
    /// Stable unique name for this process instance.
    pub consumer: String,
    pub batch_size: usize,
    pub block: Duration,
    /// Pending idle duration before another process may claim work.
    pub claim_idle: Duration,
    /// Processing lease held while the actor applies a command.
    pub lease_ttl: Duration,
    /// Terminal result and completed-idempotency retention window.
    pub completion_retention: Duration,
    pub retry_delay: Duration,
}

impl CommandWorkerConfig {
    /// # Errors
    ///
    /// Returns when a range, name, capacity, or duration is invalid.
    pub fn validate(&self) -> RuntimeResult<()> {
        if self.shards.start >= self.shards.end {
            return Err(RuntimeError::InvalidWorkerConfig(
                "shard range must be non-empty",
            ));
        }
        if !valid_name(&self.group) || !valid_name(&self.consumer) {
            return Err(RuntimeError::InvalidWorkerConfig(
                "group and consumer must be 1-128 printable characters",
            ));
        }
        if self.batch_size == 0 || self.batch_size > 1_000 {
            return Err(RuntimeError::InvalidWorkerConfig(
                "batch size must be between 1 and 1000",
            ));
        }
        if [
            self.block,
            self.claim_idle,
            self.lease_ttl,
            self.completion_retention,
            self.retry_delay,
        ]
        .into_iter()
        .any(|duration| duration.is_zero())
        {
            return Err(RuntimeError::InvalidWorkerConfig(
                "worker durations must be non-zero",
            ));
        }
        if self.lease_ttl <= self.block {
            return Err(RuntimeError::InvalidWorkerConfig(
                "processing lease must exceed the stream block duration",
            ));
        }
        Ok(())
    }
}

pub struct CommandWorkerRuntime {
    shutdown: Option<watch::Sender<bool>>,
    tasks: JoinSet<u32>,
}

impl CommandWorkerRuntime {
    /// Creates consumer groups and starts at-least-once delivery.
    ///
    /// # Errors
    ///
    /// Returns before spawning workers if validation or group creation fails.
    pub async fn start<S, D, A>(
        store: S,
        directory: Arc<D>,
        authorizer: Arc<A>,
        config: CommandWorkerConfig,
    ) -> RuntimeResult<Self>
    where
        S: CommandConsumer + CommandResultStore + IdempotencyStore + Clone + Send + Sync + 'static,
        D: PlayerDirectory,
        A: CommandAuthorizer,
    {
        config.validate()?;
        for shard_id in config.shards.clone() {
            store
                .ensure_command_group(shard_id, &config.group)
                .await
                .map_err(RuntimeError::CommandBus)?;
        }

        let (shutdown, receiver) = watch::channel(false);
        let mut tasks = JoinSet::new();
        for shard_id in config.shards.clone() {
            let store = store.clone();
            let directory = Arc::clone(&directory);
            let authorizer = Arc::clone(&authorizer);
            let config = config.clone();
            let receiver = receiver.clone();
            tasks.spawn(async move {
                run_shard(store, directory, authorizer, config, shard_id, receiver).await;
                shard_id
            });
        }
        Ok(Self {
            shutdown: Some(shutdown),
            tasks,
        })
    }

    /// Waits for the first uncoordinated shard-consumer exit, which supervisors
    /// must treat as fatal.
    pub async fn wait_for_unexpected_exit(&mut self) -> RuntimeError {
        match self.tasks.join_next().await {
            Some(Ok(shard_id)) => RuntimeError::CommandWorkerStopped { shard_id },
            Some(Err(error)) => RuntimeError::Task(error),
            None => RuntimeError::RequiredTaskStopped {
                task: "command worker set",
            },
        }
    }

    /// # Errors
    ///
    /// Returns if a shard worker panics or is cancelled.
    pub async fn shutdown(mut self) -> RuntimeResult<()> {
        if let Some(shutdown) = self.shutdown.take() {
            let _receivers = shutdown.send(true);
        }
        let mut first_error = None;
        while let Some(result) = self.tasks.join_next().await {
            if let Err(error) = result
                && first_error.is_none()
            {
                first_error = Some(RuntimeError::Task(error));
            }
        }
        first_error.map_or(Ok(()), Err)
    }
}

impl Drop for CommandWorkerRuntime {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _receivers = shutdown.send(true);
        }
        self.tasks.abort_all();
    }
}

fn valid_name(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128 && !value.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::{sync::watch, task::JoinSet};

    use super::{CommandWorkerConfig, CommandWorkerRuntime};
    use crate::RuntimeError;

    fn valid_config() -> CommandWorkerConfig {
        CommandWorkerConfig {
            shards: 0..2,
            group: "pepeaudio-bot".into(),
            consumer: "bot-0".into(),
            batch_size: 32,
            block: Duration::from_secs(1),
            claim_idle: Duration::from_secs(30),
            lease_ttl: Duration::from_secs(10),
            completion_retention: Duration::from_mins(10),
            retry_delay: Duration::from_millis(100),
        }
    }

    #[test]
    fn validates_ranges_names_and_lease_relationship() {
        assert!(valid_config().validate().is_ok());
        let mut invalid = valid_config();
        invalid.shards = 2..2;
        assert!(invalid.validate().is_err());
        let mut invalid = valid_config();
        invalid.lease_ttl = invalid.block;
        assert!(invalid.validate().is_err());
    }

    #[tokio::test]
    async fn clean_worker_return_is_an_unexpected_exit() {
        let mut tasks = JoinSet::new();
        tasks.spawn(async { 7_u32 });
        let mut runtime = CommandWorkerRuntime {
            shutdown: None,
            tasks,
        };

        assert!(matches!(
            runtime.wait_for_unexpected_exit().await,
            RuntimeError::CommandWorkerStopped { shard_id: 7 }
        ));
    }

    #[tokio::test]
    async fn worker_panic_is_an_unexpected_exit() {
        let mut tasks: JoinSet<u32> = JoinSet::new();
        tasks.spawn(async { panic!("intentional worker panic") });
        let mut runtime = CommandWorkerRuntime {
            shutdown: None,
            tasks,
        };

        let RuntimeError::Task(error) = runtime.wait_for_unexpected_exit().await else {
            panic!("worker panic must retain its JoinError");
        };
        assert!(error.is_panic());
    }

    #[tokio::test]
    async fn coordinated_shutdown_is_clean() {
        let (shutdown, mut receiver) = watch::channel(false);
        let mut tasks = JoinSet::new();
        tasks.spawn(async move {
            let _changed = receiver.changed().await;
            3_u32
        });
        let runtime = CommandWorkerRuntime {
            shutdown: Some(shutdown),
            tasks,
        };

        runtime.shutdown().await.expect("coordinated shutdown");
    }
}
