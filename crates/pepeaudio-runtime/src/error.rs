use thiserror::Error;

pub type RuntimeResult<T> = Result<T, RuntimeError>;

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("Valkey event subscription is unavailable")]
    EventSubscription(#[source] pepeaudio_storage::StorageError),
    #[error("Valkey command stream is unavailable")]
    CommandBus(#[source] pepeaudio_storage::StorageError),
    #[error("Valkey bot-presence store is unavailable")]
    PresenceStore(#[source] pepeaudio_storage::StorageError),
    #[error("bot-presence runtime stopped")]
    PresenceStopped,
    #[error("bot-presence store request timed out during {operation}")]
    PresenceTimedOut { operation: &'static str },
    #[error("runtime task failed")]
    Task(#[source] tokio::task::JoinError),
    #[error("command worker for shard {shard_id} stopped unexpectedly")]
    CommandWorkerStopped { shard_id: u32 },
    #[error("required runtime task stopped unexpectedly: {task}")]
    RequiredTaskStopped { task: &'static str },
    #[error("invalid command worker configuration: {0}")]
    InvalidWorkerConfig(&'static str),
    #[error("invalid bot-presence runtime configuration")]
    InvalidPresenceConfig,
}
