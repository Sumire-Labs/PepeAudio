use std::time::Duration;

use axum::http::{HeaderValue, Uri};
use thiserror::Error;

/// Default time for a routed command to reach its owning player worker.
pub const DEFAULT_COMMAND_TTL: Duration = Duration::from_secs(10);

/// HTTP adapter policy that must be configured at process startup.
#[derive(Clone, Debug)]
pub struct ApiConfig {
    allowed_origin: HeaderValue,
    command_ttl: Duration,
}

impl ApiConfig {
    /// `allowed_origin` must be exactly one `http` or `https` origin. Wildcard,
    /// credentials, path, query, fragment, and multiple-origin forms are
    /// rejected. This crate intentionally does not install permissive CORS.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] for a malformed origin or zero command TTL.
    pub fn new(allowed_origin: &str, command_ttl: Duration) -> Result<Self, ConfigError> {
        validate_origin(allowed_origin)?;
        if command_ttl.is_zero() {
            return Err(ConfigError::ZeroCommandTtl);
        }
        let allowed_origin =
            HeaderValue::from_str(allowed_origin).map_err(|_| ConfigError::InvalidOrigin)?;
        Ok(Self {
            allowed_origin,
            command_ttl,
        })
    }

    /// Exact origin accepted for state-changing requests.
    #[must_use]
    pub const fn allowed_origin(&self) -> &HeaderValue {
        &self.allowed_origin
    }

    /// Maximum delivery interval encoded in command envelopes.
    #[must_use]
    pub const fn command_ttl(&self) -> Duration {
        self.command_ttl
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ConfigError {
    #[error("allowed origin must be one exact http(s) origin without path, query, or fragment")]
    InvalidOrigin,
    #[error("command TTL must be greater than zero")]
    ZeroCommandTtl,
}

fn validate_origin(value: &str) -> Result<(), ConfigError> {
    if value == "*" || value.contains(',') || value.chars().any(char::is_whitespace) {
        return Err(ConfigError::InvalidOrigin);
    }
    let uri = value
        .parse::<Uri>()
        .map_err(|_| ConfigError::InvalidOrigin)?;
    let Some(scheme) = uri.scheme_str() else {
        return Err(ConfigError::InvalidOrigin);
    };
    if !matches!(scheme, "http" | "https")
        || uri.authority().is_none()
        || uri.path() != "/"
        || uri.query().is_some()
        || value.ends_with('/')
        || value.contains('#')
    {
        return Err(ConfigError::InvalidOrigin);
    }
    Ok(())
}
