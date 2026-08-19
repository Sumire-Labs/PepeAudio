use std::{num::NonZeroU32, time::Duration};

use async_trait::async_trait;
use pepeaudio_core::{CommandEnvelope, CommandResult, PlayerCommandRateLimit, shard_id};

use super::ValkeyStore;
use crate::{StorageError, StorageResult};

const COMMAND_STREAM_CAPACITY: usize = 100_000;
const ADMIT_AND_ENQUEUE_SCRIPT: &str = r"
local expected_types = {'stream', 'string', 'hash', 'hash'}
for index = 1, 4 do
  local actual_type = redis.call('TYPE', KEYS[index])['ok']
  if actual_type ~= 'none' and actual_type ~= expected_types[index] then
    return {5, tostring(index)}
  end
end

local current = redis.call('GET', KEYS[2])
if current and current ~= ARGV[3] then
  return {2, '0'}
end
if current == ARGV[3] then
  return {6, '0'}
end

if redis.call('XLEN', KEYS[1]) >= tonumber(ARGV[1]) then
  return {3, '0'}
end

local server_time = redis.call('TIME')
local now_ms = (tonumber(server_time[1]) * 1000) + math.floor(tonumber(server_time[2]) / 1000)
local window_ms = tonumber(ARGV[7])
local window_end_ms = (math.floor(now_ms / window_ms) + 1) * window_ms

local function read_counter(key)
  local stored = redis.call('HMGET', key, 'count', 'reset_at_ms')
  local count = tonumber(stored[1])
  local reset_at_ms = tonumber(stored[2])
  local ttl_ms = redis.call('PTTL', key)
  local valid = count and count >= 0 and count == math.floor(count)
    and reset_at_ms and reset_at_ms == window_end_ms
    and ttl_ms > 0 and ttl_ms <= window_ms
  if not valid then
    return 0, window_end_ms
  end
  return count, reset_at_ms
end

local actor_count, actor_reset_at = read_counter(KEYS[3])
local guild_count, guild_reset_at = read_counter(KEYS[4])
local actor_blocked = actor_count >= tonumber(ARGV[5])
local guild_blocked = guild_count >= tonumber(ARGV[6])

if actor_blocked or guild_blocked then
  local retry_at = 0
  if actor_blocked then retry_at = actor_reset_at end
  if guild_blocked and guild_reset_at > retry_at then retry_at = guild_reset_at end
  local retry_seconds = math.ceil((retry_at - now_ms) / 1000)
  if retry_seconds < 1 then retry_seconds = 1 end
  if retry_seconds > 60 then retry_seconds = 60 end
  return {4, tostring(retry_seconds)}
end

redis.call('HSET', KEYS[3], 'count', actor_count + 1, 'reset_at_ms', actor_reset_at)
redis.call('PEXPIREAT', KEYS[3], actor_reset_at)
redis.call('HSET', KEYS[4], 'count', guild_count + 1, 'reset_at_ms', guild_reset_at)
redis.call('PEXPIREAT', KEYS[4], guild_reset_at)
redis.call('SET', KEYS[2], ARGV[3], 'PX', ARGV[4])
local entry_id = redis.call('XADD', KEYS[1], '*', 'envelope', ARGV[2])
return {1, entry_id}
";

const ENQUEUED: i64 = 1;
const PENDING_CONFLICT: i64 = 2;
const STREAM_CAPACITY_EXCEEDED: i64 = 3;
const RATE_LIMITED: i64 = 4;
const WRONG_KEY_TYPE: i64 = 5;
const ALREADY_PENDING: i64 = 6;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommandEnqueue {
    /// A new stream entry and Pending result were created together.
    Enqueued {
        /// Opaque Valkey stream entry ID.
        entry_id: String,
    },
    /// The exact command ID already has its atomically-created Pending result.
    AlreadyPending,
}

/// Shard command producer used by authenticated Web/API replicas.
#[async_trait]
pub trait CommandProducer: Send + Sync {
    /// Atomically admits a command, stores its Pending result, and appends it to
    /// the owning shard stream. The authenticated envelope actor is the rate
    /// limit identity; transport forwarding headers are never consulted.
    async fn enqueue_command(
        &self,
        envelope: &CommandEnvelope,
        shard_total: u32,
        result_retention: Duration,
    ) -> StorageResult<CommandEnqueue>;
}

#[async_trait]
impl CommandProducer for ValkeyStore {
    async fn enqueue_command(
        &self,
        envelope: &CommandEnvelope,
        shard_total: u32,
        result_retention: Duration,
    ) -> StorageResult<CommandEnqueue> {
        let shard_total = NonZeroU32::new(shard_total).ok_or(StorageError::InvalidIdentifier {
            kind: "shard total",
            reason: "must be non-zero",
        })?;
        let actor_user_id = envelope
            .actor_user_id
            .ok_or(StorageError::InvalidIdentifier {
                kind: "command actor",
                reason: "must identify an authenticated user",
            })?;
        let owner = shard_id(envelope.guild_id, shard_total);
        let encoded = serde_json::to_string(envelope)?;
        let pending = serde_json::to_string(&CommandResult::pending(
            envelope.command_id,
            envelope.guild_id,
        ))?;
        let retention_ms = duration_millis(result_retention, "command result retention")?;
        let policy = PlayerCommandRateLimit::STANDARD;
        let window_ms = duration_millis(policy.window(), "player command rate window")?;
        if retention_ms == 0 || window_ms == 0 {
            return Err(StorageError::DurationOutOfRange {
                operation: "command admission",
            });
        }

        let mut connection = self.connection.clone();
        let response: (i64, String) = redis::Script::new(ADMIT_AND_ENQUEUE_SCRIPT)
            .key(self.keyspace.command_stream(owner))
            .key(
                self.keyspace
                    .command_result(envelope.guild_id, envelope.command_id),
            )
            .key(
                self.keyspace
                    .player_command_actor_rate_limit(envelope.guild_id, actor_user_id),
            )
            .key(
                self.keyspace
                    .player_command_guild_rate_limit(envelope.guild_id),
            )
            .arg(COMMAND_STREAM_CAPACITY)
            .arg(encoded)
            .arg(pending)
            .arg(retention_ms)
            .arg(policy.per_actor_per_guild())
            .arg(policy.per_guild())
            .arg(window_ms)
            .invoke_async(&mut connection)
            .await?;
        decode_response(response)
    }
}

fn decode_response((status, value): (i64, String)) -> StorageResult<CommandEnqueue> {
    match status {
        ENQUEUED if !value.is_empty() => Ok(CommandEnqueue::Enqueued { entry_id: value }),
        PENDING_CONFLICT => Err(StorageError::CorruptData {
            entity: "command result",
            field: "pending_conflict",
        }),
        STREAM_CAPACITY_EXCEEDED => Err(StorageError::CapacityExceeded {
            resource: "shard command stream",
        }),
        RATE_LIMITED => {
            let seconds = value
                .parse::<u64>()
                .ok()
                .filter(|seconds| (1..=60).contains(seconds))
                .ok_or(StorageError::CorruptData {
                    entity: "command admission",
                    field: "retry_after",
                })?;
            Err(StorageError::RateLimited {
                resource: "player command admission",
                retry_after: Duration::from_secs(seconds),
            })
        }
        WRONG_KEY_TYPE => Err(StorageError::CorruptData {
            entity: "command admission",
            field: "key_type",
        }),
        ALREADY_PENDING => Ok(CommandEnqueue::AlreadyPending),
        _ => Err(StorageError::CorruptData {
            entity: "command admission",
            field: "script_response",
        }),
    }
}

fn duration_millis(duration: Duration, operation: &'static str) -> StorageResult<usize> {
    usize::try_from(duration.as_millis())
        .map_err(|_| StorageError::DurationOutOfRange { operation })
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{RATE_LIMITED, decode_response};
    use crate::StorageError;

    #[test]
    fn rejects_corrupt_retry_delays_outside_the_public_contract() {
        for value in ["0", "61", "not-a-number"] {
            assert!(matches!(
                decode_response((RATE_LIMITED, value.to_owned())),
                Err(StorageError::CorruptData {
                    entity: "command admission",
                    field: "retry_after"
                })
            ));
        }

        assert!(matches!(
            decode_response((RATE_LIMITED, "17".to_owned())),
            Err(StorageError::RateLimited {
                retry_after,
                ..
            }) if retry_after == Duration::from_secs(17)
        ));
    }
}
