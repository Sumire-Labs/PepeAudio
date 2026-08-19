use std::time::Duration;

use async_trait::async_trait;
use pepeaudio_core::GuildId;
use redis::Script;

use crate::{StorageError, StorageResult, valkey::ValkeyStore};

const CLEAR_IF_OWNER: &str = r"
local current = redis.call('GET', KEYS[1])
if current == ARGV[1] then
  return redis.call('DEL', KEYS[1])
end
return 0
";

/// Short-lived gateway membership advertised by the owning shard process.
#[async_trait]
pub trait BotPresenceStore: Send + Sync {
    async fn refresh_bot_presence(
        &self,
        guild_id: GuildId,
        instance_id: &str,
        ttl: Duration,
    ) -> StorageResult<()>;
    /// Clears presence only when the named process still owns it.
    async fn clear_bot_presence(&self, guild_id: GuildId, instance_id: &str)
    -> StorageResult<bool>;
    async fn is_bot_present(&self, guild_id: GuildId) -> StorageResult<bool>;
}

#[async_trait]
impl BotPresenceStore for ValkeyStore {
    async fn refresh_bot_presence(
        &self,
        guild_id: GuildId,
        instance_id: &str,
        ttl: Duration,
    ) -> StorageResult<()> {
        validate_instance(instance_id)?;
        let ttl_ms = duration_ms(ttl)?;
        let mut connection = self.connection.clone();
        let response: Option<String> = redis::cmd("SET")
            .arg(self.keyspace.bot_presence(guild_id))
            .arg(instance_id)
            .arg("PX")
            .arg(ttl_ms)
            .query_async(&mut connection)
            .await?;
        if response.as_deref() == Some("OK") {
            Ok(())
        } else {
            Err(StorageError::CorruptData {
                entity: "bot_presence",
                field: "set_response",
            })
        }
    }

    async fn clear_bot_presence(
        &self,
        guild_id: GuildId,
        instance_id: &str,
    ) -> StorageResult<bool> {
        validate_instance(instance_id)?;
        let mut connection = self.connection.clone();
        let deleted: i64 = Script::new(CLEAR_IF_OWNER)
            .key(self.keyspace.bot_presence(guild_id))
            .arg(instance_id)
            .invoke_async(&mut connection)
            .await?;
        Ok(deleted == 1)
    }

    async fn is_bot_present(&self, guild_id: GuildId) -> StorageResult<bool> {
        let mut connection = self.connection.clone();
        let exists: bool = redis::cmd("EXISTS")
            .arg(self.keyspace.bot_presence(guild_id))
            .query_async(&mut connection)
            .await?;
        Ok(exists)
    }
}

fn validate_instance(value: &str) -> StorageResult<()> {
    let valid = !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'));
    if valid {
        Ok(())
    } else {
        Err(StorageError::InvalidIdentifier {
            kind: "presence instance",
            reason: "must be 1-64 ASCII letters, digits, dots, underscores, or hyphens",
        })
    }
}

fn duration_ms(duration: Duration) -> StorageResult<u64> {
    u64::try_from(duration.as_millis())
        .ok()
        .filter(|value| *value != 0)
        .ok_or(StorageError::DurationOutOfRange {
            operation: "bot presence",
        })
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{duration_ms, validate_instance};

    #[test]
    fn validates_instance_and_ttl_before_redis() {
        assert!(validate_instance("bot-1.example").is_ok());
        assert!(validate_instance("bad instance").is_err());
        assert!(duration_ms(Duration::ZERO).is_err());
        assert_eq!(duration_ms(Duration::from_secs(2)).expect("TTL"), 2_000);
    }
}
