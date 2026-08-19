use std::{fmt, time::Duration};

use sqlx::{PgPool, postgres::PgPoolOptions};

use crate::StorageResult;

const POOL_ACQUIRE_TIMEOUT: Duration = Duration::from_secs(5);
const STATEMENT_TIMEOUT_SQL: &str = "SET statement_timeout = '10s'";
const LOCK_TIMEOUT_SQL: &str = "SET lock_timeout = '5s'";
const IDLE_TRANSACTION_TIMEOUT_SQL: &str = "SET idle_in_transaction_session_timeout = '15s'";

#[derive(Clone)]
pub struct PostgresStorage {
    pub(crate) pool: PgPool,
}

impl PostgresStorage {
    /// Connects a bounded pool. The URL is consumed by `SQLx` and never retained
    /// in this type's formatting output.
    ///
    /// # Errors
    ///
    /// Returns an error when initial connection establishment fails.
    pub async fn connect(database_url: &str, max_connections: u32) -> StorageResult<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(max_connections.max(1))
            .acquire_timeout(POOL_ACQUIRE_TIMEOUT)
            .after_connect(|connection, _metadata| {
                Box::pin(async move {
                    sqlx::query(STATEMENT_TIMEOUT_SQL)
                        .execute(&mut *connection)
                        .await?;
                    sqlx::query(LOCK_TIMEOUT_SQL)
                        .execute(&mut *connection)
                        .await?;
                    sqlx::query(IDLE_TRANSACTION_TIMEOUT_SQL)
                        .execute(&mut *connection)
                        .await?;
                    Ok(())
                })
            })
            .connect(database_url)
            .await?;
        Ok(Self { pool })
    }

    #[must_use]
    pub const fn from_pool(pool: PgPool) -> Self {
        Self { pool }
    }

    #[must_use]
    pub const fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Sends a lightweight health probe through the pool.
    ///
    /// # Errors
    ///
    /// Returns an error when `PostgreSQL` is unavailable.
    pub async fn ping(&self) -> StorageResult<()> {
        sqlx::query("SELECT 1").execute(&self.pool).await?;
        Ok(())
    }

    pub async fn close(&self) {
        self.pool.close().await;
    }
}

impl fmt::Debug for PostgresStorage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PostgresStorage")
            .field("size", &self.pool.size())
            .field("idle", &self.pool.num_idle())
            .finish_non_exhaustive()
    }
}
