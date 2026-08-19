use std::time::Duration;

use thiserror::Error;

pub type StorageResult<T> = Result<T, StorageError>;

/// Backend or stored-data failure without connection URLs or credentials.
#[derive(Debug, Error)]
pub enum StorageError {
    #[error("PostgreSQL operation failed")]
    Database(#[source] sqlx::Error),
    #[error("Valkey operation failed")]
    Valkey(#[source] redis::RedisError),
    #[error("stored JSON payload is invalid")]
    Json(#[source] serde_json::Error),
    /// A database row violated assumptions guaranteed by the migration.
    #[error("stored {entity} has an invalid {field}")]
    CorruptData {
        /// Stable entity name suitable for a low-cardinality error code.
        entity: &'static str,
        /// Stable field name; never a stored value.
        field: &'static str,
    },
    #[error("invalid {kind}: {reason}")]
    InvalidIdentifier {
        kind: &'static str,
        /// Non-sensitive validation detail.
        reason: &'static str,
    },
    #[error("{operation} duration is outside the supported range")]
    DurationOutOfRange {
        /// Stable operation name; never a duration or caller-supplied value.
        operation: &'static str,
    },
    #[error("{resource} reached its configured capacity")]
    CapacityExceeded {
        /// Stable resource name; never a user or stored value.
        resource: &'static str,
    },
    #[error("{resource} rate limit exceeded")]
    RateLimited {
        /// Stable resource name; never an actor, guild, or backend key.
        resource: &'static str,
        /// Server-authoritative delay before the request may be retried.
        retry_after: Duration,
    },
}

impl From<sqlx::Error> for StorageError {
    fn from(error: sqlx::Error) -> Self {
        Self::Database(error)
    }
}

impl From<redis::RedisError> for StorageError {
    fn from(error: redis::RedisError) -> Self {
        Self::Valkey(error)
    }
}

impl From<serde_json::Error> for StorageError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}
