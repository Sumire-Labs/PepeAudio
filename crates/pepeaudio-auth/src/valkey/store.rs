use std::{fmt, sync::Arc};

use redis::{Client, aio::ConnectionManager};

use crate::{AuthClock, AuthConfig, RepositoryError, SystemAuthClock};

pub(crate) const MAX_PENDING_OAUTH_STATES: u32 = 4_096;

/// Valkey-backed one-time OAuth state and opaque session repository.
#[derive(Clone)]
pub struct ValkeyAuthStore {
    pub(crate) connection: ConnectionManager,
    pub(crate) keyspace: Arc<str>,
    pub(crate) state_ttl_ms: u64,
    pub(crate) idle_ttl_ms: u64,
    pub(crate) absolute_ttl_ms: u64,
    pub(crate) clock: Arc<dyn AuthClock>,
}

impl ValkeyAuthStore {
    /// # Errors
    ///
    /// Returns a redacted repository error when URL parsing or connection fails.
    pub async fn connect(valkey_url: &str, config: &AuthConfig) -> Result<Self, RepositoryError> {
        let client = Client::open(valkey_url).map_err(|_| RepositoryError::Unavailable)?;
        let connection = client
            .get_connection_manager()
            .await
            .map_err(|_| RepositoryError::Unavailable)?;
        Self::from_connection(connection, config, Arc::new(SystemAuthClock))
    }

    /// # Errors
    ///
    /// Rejects session durations that cannot be represented as milliseconds.
    pub fn from_connection(
        connection: ConnectionManager,
        config: &AuthConfig,
        clock: Arc<dyn AuthClock>,
    ) -> Result<Self, RepositoryError> {
        Ok(Self {
            connection,
            keyspace: Arc::from(config.keyspace.as_str()),
            state_ttl_ms: duration_ms(config.session.oauth_state_ttl)?,
            idle_ttl_ms: duration_ms(config.session.idle_ttl)?,
            absolute_ttl_ms: duration_ms(config.session.absolute_ttl)?,
            clock,
        })
    }

    /// # Errors
    ///
    /// Returns a redacted unavailable error when Valkey cannot answer `PING`.
    pub async fn ping(&self) -> Result<(), RepositoryError> {
        let mut connection = self.connection.clone();
        let response: String = redis::cmd("PING")
            .query_async(&mut connection)
            .await
            .map_err(|_| RepositoryError::Unavailable)?;
        if response == "PONG" {
            Ok(())
        } else {
            Err(RepositoryError::Unavailable)
        }
    }

    pub(crate) fn state_key(&self, state_hash: &str) -> String {
        format!("{}:oauth-state:{state_hash}", self.keyspace)
    }

    pub(crate) fn pending_states_key(&self) -> String {
        format!("{}:oauth-pending-states", self.keyspace)
    }

    pub(crate) fn session_key(&self, session_hash: &str) -> String {
        format!("{}:session:{session_hash}", self.keyspace)
    }

    pub(crate) fn current_user_key(&self, user_id: pepeaudio_core::UserId) -> String {
        format!("{}:user-current-session:{user_id}", self.keyspace)
    }
}

impl fmt::Debug for ValkeyAuthStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ValkeyAuthStore")
            .field("keyspace", &self.keyspace)
            .field("state_ttl_ms", &self.state_ttl_ms)
            .field("idle_ttl_ms", &self.idle_ttl_ms)
            .field("absolute_ttl_ms", &self.absolute_ttl_ms)
            .finish_non_exhaustive()
    }
}

fn duration_ms(duration: std::time::Duration) -> Result<u64, RepositoryError> {
    u64::try_from(duration.as_millis())
        .ok()
        .filter(|value| *value != 0)
        .ok_or(RepositoryError::Corrupt)
}
