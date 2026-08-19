use std::fmt;

use redis::{Client, aio::ConnectionManager};

use super::Keyspace;
use crate::StorageResult;

/// Cloneable Valkey interface backed by a reconnecting multiplexed connection.
#[derive(Clone)]
pub struct ValkeyStore {
    pub(super) client: Client,
    pub(super) connection: ConnectionManager,
    pub(super) keyspace: Keyspace,
}

impl ValkeyStore {
    /// Connects through a reconnecting connection manager.
    ///
    /// # Errors
    ///
    /// Returns an error when URL parsing or the initial connection fails.
    pub async fn connect(valkey_url: &str, keyspace: Keyspace) -> StorageResult<Self> {
        let client = Client::open(valkey_url)?;
        let connection = client.get_connection_manager().await?;
        Ok(Self {
            client,
            connection,
            keyspace,
        })
    }

    #[must_use]
    pub const fn from_connection(
        client: Client,
        connection: ConnectionManager,
        keyspace: Keyspace,
    ) -> Self {
        Self {
            client,
            connection,
            keyspace,
        }
    }

    /// Sends a lightweight health probe.
    ///
    /// # Errors
    ///
    /// Returns an error when Valkey is unavailable or returns an invalid reply.
    pub async fn ping(&self) -> StorageResult<()> {
        let mut connection = self.connection.clone();
        let response: String = redis::cmd("PING").query_async(&mut connection).await?;
        if response == "PONG" {
            Ok(())
        } else {
            Err(crate::StorageError::CorruptData {
                entity: "valkey",
                field: "ping_response",
            })
        }
    }
}

impl fmt::Debug for ValkeyStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ValkeyStore")
            .field("keyspace", &self.keyspace)
            .finish_non_exhaustive()
    }
}
