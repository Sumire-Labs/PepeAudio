use std::time::Duration;

use async_trait::async_trait;
use pepeaudio_core::{CommandResult, GuildId};
use redis::AsyncCommands as _;
use uuid::Uuid;

use super::{ValkeyStore, dedupe::lease_value};
use crate::{StorageError, StorageResult};

const STORE_TERMINAL_SCRIPT: &str = r"
local current = redis.call('GET', KEYS[1])
if current == ARGV[1] then
  redis.call('PEXPIRE', KEYS[1], ARGV[2])
  return 2
end
if current then
  local decoded = cjson.decode(current)
  if decoded.command_id ~= ARGV[3] or decoded.guild_id ~= ARGV[4] or decoded.status ~= 'pending' then
    return 3
  end
end
redis.call('SET', KEYS[1], ARGV[1], 'PX', ARGV[2])
return 1
";

const COMPLETE_WITH_RESULT_SCRIPT: &str = r"
if redis.call('GET', KEYS[1]) ~= ARGV[1] then
  return 0
end
local current = redis.call('GET', KEYS[2])
if current == ARGV[2] then
  redis.call('SET', KEYS[1], 'done', 'PX', ARGV[3])
  redis.call('PEXPIRE', KEYS[2], ARGV[4])
  return 2
end
if current then
  local decoded = cjson.decode(current)
  if decoded.command_id ~= ARGV[5] or decoded.guild_id ~= ARGV[6] or decoded.status ~= 'pending' then
    return 3
  end
end
redis.call('SET', KEYS[2], ARGV[2], 'PX', ARGV[4])
redis.call('SET', KEYS[1], 'done', 'PX', ARGV[3])
return 1
";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandResultWrite {
    /// A pending or absent record became terminal.
    Stored,
    /// The exact terminal result had already been stored.
    Unchanged,
    /// Another terminal result already owns this command ID.
    Conflict,
}

/// Result of atomically completing an idempotency lease and command result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandCompletion {
    Stored,
    Unchanged,
    LeaseLost,
    Conflict,
}

/// Short-lived command status storage shared by API and command workers.
#[async_trait]
pub trait CommandResultStore: Send + Sync {
    async fn get_command_result(
        &self,
        guild_id: GuildId,
        command_id: Uuid,
    ) -> StorageResult<Option<CommandResult>>;

    /// Persists a terminal result, replacing only the pending marker.
    async fn store_command_result(
        &self,
        result: &CommandResult,
        retention: Duration,
    ) -> StorageResult<CommandResultWrite>;

    /// Atomically persists a terminal result and marks the caller-owned lease done.
    async fn complete_command(
        &self,
        idempotency_key: Uuid,
        lease_token: Uuid,
        result: &CommandResult,
        idempotency_retention: Duration,
        result_retention: Duration,
    ) -> StorageResult<CommandCompletion>;
}

#[async_trait]
impl CommandResultStore for ValkeyStore {
    async fn get_command_result(
        &self,
        guild_id: GuildId,
        command_id: Uuid,
    ) -> StorageResult<Option<CommandResult>> {
        let mut connection = self.connection.clone();
        let encoded: Option<String> = connection
            .get(self.keyspace.command_result(guild_id, command_id))
            .await?;
        let Some(encoded) = encoded else {
            return Ok(None);
        };
        let result: CommandResult = serde_json::from_str(&encoded)?;
        validate_identity(&result, guild_id, command_id)?;
        Ok(Some(result))
    }

    async fn store_command_result(
        &self,
        result: &CommandResult,
        retention: Duration,
    ) -> StorageResult<CommandResultWrite> {
        require_terminal(result)?;
        let retention_ms = duration_millis(retention, "command result retention")?;
        let encoded = serde_json::to_string(result)?;
        let mut connection = self.connection.clone();
        let status: i32 = redis::Script::new(STORE_TERMINAL_SCRIPT)
            .key(
                self.keyspace
                    .command_result(result.guild_id, result.command_id),
            )
            .arg(encoded)
            .arg(retention_ms)
            .arg(result.command_id.to_string())
            .arg(result.guild_id.to_string())
            .invoke_async(&mut connection)
            .await?;
        match status {
            1 => Ok(CommandResultWrite::Stored),
            2 => Ok(CommandResultWrite::Unchanged),
            3 => Ok(CommandResultWrite::Conflict),
            _ => Err(corrupt_status()),
        }
    }

    async fn complete_command(
        &self,
        idempotency_key: Uuid,
        lease_token: Uuid,
        result: &CommandResult,
        idempotency_retention: Duration,
        result_retention: Duration,
    ) -> StorageResult<CommandCompletion> {
        require_terminal(result)?;
        let idempotency_ms = duration_millis(
            idempotency_retention,
            "command idempotency completion retention",
        )?;
        let result_ms = duration_millis(result_retention, "command result retention")?;
        let encoded = serde_json::to_string(result)?;
        let mut connection = self.connection.clone();
        let status: i32 = redis::Script::new(COMPLETE_WITH_RESULT_SCRIPT)
            .key(self.keyspace.dedupe(result.guild_id, idempotency_key))
            .key(
                self.keyspace
                    .command_result(result.guild_id, result.command_id),
            )
            .arg(lease_value(lease_token))
            .arg(encoded)
            .arg(idempotency_ms)
            .arg(result_ms)
            .arg(result.command_id.to_string())
            .arg(result.guild_id.to_string())
            .invoke_async(&mut connection)
            .await?;
        match status {
            0 => Ok(CommandCompletion::LeaseLost),
            1 => Ok(CommandCompletion::Stored),
            2 => Ok(CommandCompletion::Unchanged),
            3 => Ok(CommandCompletion::Conflict),
            _ => Err(corrupt_status()),
        }
    }
}

fn validate_identity(
    result: &CommandResult,
    guild_id: GuildId,
    command_id: Uuid,
) -> StorageResult<()> {
    if result.guild_id != guild_id || result.command_id != command_id {
        Err(StorageError::CorruptData {
            entity: "command result",
            field: "identity",
        })
    } else {
        Ok(())
    }
}

fn require_terminal(result: &CommandResult) -> StorageResult<()> {
    if result.is_terminal() {
        Ok(())
    } else {
        Err(StorageError::CorruptData {
            entity: "command result",
            field: "terminal_status",
        })
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

const fn corrupt_status() -> StorageError {
    StorageError::CorruptData {
        entity: "command result",
        field: "write_status",
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::duration_millis;

    #[test]
    fn result_retention_must_be_positive_and_representable() {
        assert!(duration_millis(Duration::ZERO, "test").is_err());
        assert_eq!(
            duration_millis(Duration::from_millis(1), "test").expect("duration"),
            1
        );
    }
}
