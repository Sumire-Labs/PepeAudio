use std::{collections::HashMap, fmt, sync::Arc};

use redis::{
    Client,
    aio::{ConnectionManager, ConnectionManagerConfig},
};
use tokio::sync::Mutex;

use super::Keyspace;
use crate::StorageResult;

/// Cloneable Valkey interface backed by a reconnecting multiplexed connection.
#[derive(Clone)]
pub struct ValkeyStore {
    pub(super) client: Client,
    pub(super) connection: ConnectionManager,
    blocking_connections: Arc<Mutex<HashMap<u32, ConnectionManager>>>,
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
            blocking_connections: Arc::new(Mutex::new(HashMap::new())),
            keyspace,
        })
    }

    #[must_use]
    pub fn from_connection(
        client: Client,
        connection: ConnectionManager,
        keyspace: Keyspace,
    ) -> Self {
        Self {
            client,
            connection,
            blocking_connections: Arc::new(Mutex::new(HashMap::new())),
            keyspace,
        }
    }

    pub(super) async fn blocking_connection(
        &self,
        shard_id: u32,
    ) -> StorageResult<ConnectionManager> {
        if let Some(connection) = self.blocking_connections.lock().await.get(&shard_id) {
            return Ok(connection.clone());
        }

        // A server-bounded XREADGROUP may legitimately exceed redis-rs's
        // 500 ms default response timeout. Isolate it so it cannot block or
        // reconnect the connection used by snapshots, results, and presence.
        let config = ConnectionManagerConfig::new().set_response_timeout(None);
        let candidate = self
            .client
            .get_connection_manager_with_config(config)
            .await?;
        let mut connections = self.blocking_connections.lock().await;
        Ok(connections.entry(shard_id).or_insert(candidate).clone())
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
