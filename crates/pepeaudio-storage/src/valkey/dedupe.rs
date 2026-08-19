use std::time::Duration;

use async_trait::async_trait;
use pepeaudio_core::GuildId;
use uuid::Uuid;

use super::ValkeyStore;
use crate::{StorageError, StorageResult};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DedupeClaim {
    /// This caller owns the lease and may apply the operation.
    Acquired,
    /// Another worker currently owns an unexpired processing lease.
    InProgress,
    /// The logical operation has already completed.
    Completed,
}

/// Recoverable idempotency state for at-least-once command delivery.
#[async_trait]
pub trait IdempotencyStore: Send + Sync {
    /// Acquires a short processing lease for one logical mutation.
    async fn claim_idempotency(
        &self,
        guild_id: GuildId,
        idempotency_key: Uuid,
        lease_token: Uuid,
        lease_ttl: Duration,
    ) -> StorageResult<DedupeClaim>;

    /// Marks a caller-owned lease completed for the retry retention window.
    async fn complete_idempotency(
        &self,
        guild_id: GuildId,
        idempotency_key: Uuid,
        lease_token: Uuid,
        retention: Duration,
    ) -> StorageResult<bool>;

    /// Releases a caller-owned lease after a failed, unapplied operation.
    async fn release_idempotency(
        &self,
        guild_id: GuildId,
        idempotency_key: Uuid,
        lease_token: Uuid,
    ) -> StorageResult<bool>;
}

const CLAIM_SCRIPT: &str = r"
local current = redis.call('GET', KEYS[1])
if not current then
  redis.call('SET', KEYS[1], ARGV[1], 'PX', ARGV[2])
  return 1
end
if current == 'done' then
  return 3
end
return 2
";

const COMPLETE_SCRIPT: &str = r"
if redis.call('GET', KEYS[1]) ~= ARGV[1] then
  return 0
end
redis.call('SET', KEYS[1], 'done', 'PX', ARGV[2])
return 1
";

const RELEASE_SCRIPT: &str = r"
if redis.call('GET', KEYS[1]) ~= ARGV[1] then
  return 0
end
return redis.call('DEL', KEYS[1])
";

#[async_trait]
impl IdempotencyStore for ValkeyStore {
    async fn claim_idempotency(
        &self,
        guild_id: GuildId,
        idempotency_key: Uuid,
        lease_token: Uuid,
        lease_ttl: Duration,
    ) -> StorageResult<DedupeClaim> {
        let lease_ms = duration_millis(lease_ttl, "idempotency lease")?;
        let mut connection = self.connection.clone();
        let status: i32 = redis::Script::new(CLAIM_SCRIPT)
            .key(self.keyspace.dedupe(guild_id, idempotency_key))
            .arg(lease_value(lease_token))
            .arg(lease_ms)
            .invoke_async(&mut connection)
            .await?;
        match status {
            1 => Ok(DedupeClaim::Acquired),
            2 => Ok(DedupeClaim::InProgress),
            3 => Ok(DedupeClaim::Completed),
            _ => Err(StorageError::CorruptData {
                entity: "idempotency",
                field: "claim_status",
            }),
        }
    }

    async fn complete_idempotency(
        &self,
        guild_id: GuildId,
        idempotency_key: Uuid,
        lease_token: Uuid,
        retention: Duration,
    ) -> StorageResult<bool> {
        let retention_ms = duration_millis(retention, "idempotency retention")?;
        let mut connection = self.connection.clone();
        let completed: i32 = redis::Script::new(COMPLETE_SCRIPT)
            .key(self.keyspace.dedupe(guild_id, idempotency_key))
            .arg(lease_value(lease_token))
            .arg(retention_ms)
            .invoke_async(&mut connection)
            .await?;
        Ok(completed == 1)
    }

    async fn release_idempotency(
        &self,
        guild_id: GuildId,
        idempotency_key: Uuid,
        lease_token: Uuid,
    ) -> StorageResult<bool> {
        let mut connection = self.connection.clone();
        let released: i32 = redis::Script::new(RELEASE_SCRIPT)
            .key(self.keyspace.dedupe(guild_id, idempotency_key))
            .arg(lease_value(lease_token))
            .invoke_async(&mut connection)
            .await?;
        Ok(released == 1)
    }
}

fn duration_millis(duration: Duration, operation: &'static str) -> StorageResult<u64> {
    let millis = u64::try_from(duration.as_millis())
        .map_err(|_| StorageError::DurationOutOfRange { operation })?;
    if millis == 0 {
        Err(StorageError::DurationOutOfRange { operation })
    } else {
        Ok(millis)
    }
}

pub(super) fn lease_value(token: Uuid) -> String {
    format!("lease:{token}")
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use uuid::Uuid;

    use super::{duration_millis, lease_value};

    #[test]
    fn validates_lease_duration_and_prefixes_lease_tokens() {
        assert!(duration_millis(Duration::ZERO, "test").is_err());
        assert_eq!(
            duration_millis(Duration::from_millis(1), "test").expect("one millisecond"),
            1
        );
        assert_eq!(
            lease_value(Uuid::nil()),
            "lease:00000000-0000-0000-0000-000000000000"
        );
    }
}
