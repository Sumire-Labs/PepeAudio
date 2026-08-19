use thiserror::Error;

pub type ConfigResult<T> = Result<T, ConfigError>;

/// A configuration error that identifies the variable without exposing its value.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ConfigError {
    #[error("required environment variable {name} is missing")]
    Missing { name: &'static str },
    #[error("required secret {name} or {file_name} is missing")]
    MissingSecret {
        name: &'static str,
        file_name: &'static str,
    },
    #[error("secret variables {name} and {file_name} cannot both be set")]
    ConflictingSecretSources {
        name: &'static str,
        file_name: &'static str,
    },
    /// A secret file could not be read. Its path and contents are omitted.
    #[error("secret file configured by {name} could not be read")]
    SecretFile { name: &'static str },
    #[error("environment variable {name} is not valid Unicode")]
    NotUnicode { name: &'static str },
    /// A value was present but invalid. The value itself is intentionally omitted.
    #[error("environment variable {name} is invalid: {reason}")]
    Invalid {
        name: &'static str,
        reason: &'static str,
    },
    #[error("configuration is inconsistent: {reason}")]
    Inconsistent { reason: &'static str },
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ShardConfigError {
    #[error("shard total must be greater than zero")]
    ZeroTotal,
    #[error("shard range must be non-empty: {start}..{end_exclusive}")]
    EmptyOrInvertedRange { start: u32, end_exclusive: u32 },
    #[error("shard range end {end_exclusive} exceeds total {total}")]
    OutOfRange { end_exclusive: u32, total: u32 },
}
