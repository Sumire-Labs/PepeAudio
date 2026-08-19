use std::{pin::Pin, time::Duration};

use async_trait::async_trait;
use futures_util::{Stream, StreamExt as _};
use pepeaudio_core::{
    GuildId, MAX_PLAYER_SNAPSHOT_JSON_BYTES, PlayerSnapshot, PlayerSnapshotValidationError,
    StateRevision,
};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};

use super::ValkeyStore;
use crate::{StorageError, StorageResult};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SnapshotWrite {
    /// The new snapshot was accepted and an event was published.
    Stored,
    /// The stored revision was equal to or newer than the attempted revision.
    Stale,
}

/// Small Pub/Sub notification; consumers fetch the full snapshot separately.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SnapshotEvent {
    pub guild_id: GuildId,
    pub revision: StateRevision,
}

/// Pub/Sub is only a wake-up signal. Consumers must always fetch and compare
/// the versioned full snapshot before exposing new state.
pub type SnapshotEventStream =
    Pin<Box<dyn Stream<Item = StorageResult<SnapshotEvent>> + Send + 'static>>;

#[async_trait]
pub trait SnapshotEventSubscriber: Send + Sync {
    async fn subscribe_snapshot_events(&self) -> StorageResult<SnapshotEventStream>;
}

#[async_trait]
pub trait SnapshotStore: Send + Sync {
    async fn get_snapshot(&self, guild_id: GuildId) -> StorageResult<Option<PlayerSnapshot>>;

    /// Gets the durable monotonic revision watermark for a guild actor.
    async fn get_snapshot_revision(
        &self,
        guild_id: GuildId,
    ) -> StorageResult<Option<StateRevision>>;

    /// Removes only the disposable snapshot body, preserving its revision clock.
    async fn invalidate_snapshot(&self, guild_id: GuildId) -> StorageResult<()>;

    /// Stores only a strictly newer revision and publishes a compact event.
    async fn put_snapshot_if_newer(
        &self,
        snapshot: &PlayerSnapshot,
        ttl: Duration,
    ) -> StorageResult<SnapshotWrite>;
}

const PUT_SNAPSHOT_SCRIPT: &str = r"
local current = redis.call('GET', KEYS[2])
if current and current >= ARGV[1] then
  return 0
end
redis.call('SET', KEYS[1], ARGV[2], 'PX', ARGV[3])
redis.call('SET', KEYS[2], ARGV[1])
redis.call('PUBLISH', KEYS[3], ARGV[4])
return 1
";

const GET_SNAPSHOT_SCRIPT: &str = r"
local key_type = redis.call('TYPE', KEYS[1]).ok
if key_type == 'none' then
  return {0, ''}
end
if key_type ~= 'string' then
  return {3, ''}
end
if redis.call('STRLEN', KEYS[1]) > tonumber(ARGV[1]) then
  return {1, ''}
end
return {2, redis.call('GET', KEYS[1])}
";

#[async_trait]
impl SnapshotStore for ValkeyStore {
    async fn get_snapshot(&self, guild_id: GuildId) -> StorageResult<Option<PlayerSnapshot>> {
        let key = self.keyspace.snapshot(guild_id);
        let mut connection = self.connection.clone();
        let (status, encoded): (u8, Vec<u8>) = redis::Script::new(GET_SNAPSHOT_SCRIPT)
            .key(key)
            .arg(MAX_PLAYER_SNAPSHOT_JSON_BYTES)
            .invoke_async(&mut connection)
            .await?;
        match status {
            0 => Ok(None),
            1 => Err(StorageError::CorruptData {
                entity: "player_snapshot",
                field: "encoded_size",
            }),
            2 => decode_snapshot(guild_id, &encoded).map(Some),
            3 => Err(StorageError::CorruptData {
                entity: "player_snapshot",
                field: "key_type",
            }),
            _ => Err(StorageError::CorruptData {
                entity: "player_snapshot",
                field: "read_status",
            }),
        }
    }

    async fn get_snapshot_revision(
        &self,
        guild_id: GuildId,
    ) -> StorageResult<Option<StateRevision>> {
        let key = self.keyspace.snapshot_revision(guild_id);
        let mut connection = self.connection.clone();
        let encoded: Option<String> = connection.get(key).await?;
        encoded
            .map(|value| parse_revision_sort_key(&value))
            .transpose()
    }

    async fn invalidate_snapshot(&self, guild_id: GuildId) -> StorageResult<()> {
        let key = self.keyspace.snapshot(guild_id);
        let mut connection = self.connection.clone();
        let _deleted: u64 = connection.del(key).await?;
        Ok(())
    }

    async fn put_snapshot_if_newer(
        &self,
        snapshot: &PlayerSnapshot,
        ttl: Duration,
    ) -> StorageResult<SnapshotWrite> {
        let ttl_ms =
            u64::try_from(ttl.as_millis()).map_err(|_| StorageError::DurationOutOfRange {
                operation: "snapshot TTL",
            })?;
        if ttl_ms == 0 {
            return Err(StorageError::DurationOutOfRange {
                operation: "snapshot TTL",
            });
        }
        snapshot
            .validate_public_shape()
            .map_err(snapshot_shape_error)?;
        let encoded = serde_json::to_string(snapshot)?;
        if encoded.len() > MAX_PLAYER_SNAPSHOT_JSON_BYTES {
            return Err(StorageError::CapacityExceeded {
                resource: "player snapshot",
            });
        }
        let event = serde_json::to_string(&SnapshotEvent {
            guild_id: snapshot.guild_id,
            revision: snapshot.revision,
        })?;
        let mut connection = self.connection.clone();
        let stored: i32 = redis::Script::new(PUT_SNAPSHOT_SCRIPT)
            .key(self.keyspace.snapshot(snapshot.guild_id))
            .key(self.keyspace.snapshot_revision(snapshot.guild_id))
            .key(self.keyspace.snapshot_event(snapshot.guild_id))
            .arg(revision_sort_key(snapshot.revision))
            .arg(encoded)
            .arg(ttl_ms)
            .arg(event)
            .invoke_async(&mut connection)
            .await?;
        Ok(if stored == 1 {
            SnapshotWrite::Stored
        } else {
            SnapshotWrite::Stale
        })
    }
}

fn decode_snapshot(guild_id: GuildId, encoded: &[u8]) -> StorageResult<PlayerSnapshot> {
    if encoded.len() > MAX_PLAYER_SNAPSHOT_JSON_BYTES {
        return Err(StorageError::CorruptData {
            entity: "player_snapshot",
            field: "encoded_size",
        });
    }
    let snapshot = serde_json::from_slice::<PlayerSnapshot>(encoded)?;
    if snapshot.guild_id != guild_id {
        return Err(StorageError::CorruptData {
            entity: "player_snapshot",
            field: "guild_id",
        });
    }
    snapshot
        .validate_public_shape()
        .map_err(snapshot_shape_error)?;
    Ok(snapshot)
}

fn snapshot_shape_error(error: PlayerSnapshotValidationError) -> StorageError {
    StorageError::CorruptData {
        entity: "player_snapshot",
        field: error.field(),
    }
}

#[async_trait]
impl SnapshotEventSubscriber for ValkeyStore {
    async fn subscribe_snapshot_events(&self) -> StorageResult<SnapshotEventStream> {
        let mut pubsub = self.client.get_async_pubsub().await?;
        pubsub
            .psubscribe(self.keyspace.snapshot_event_pattern())
            .await?;
        let messages = pubsub.into_on_message().map(|message| {
            let payload = message.get_payload::<String>()?;
            serde_json::from_str::<SnapshotEvent>(&payload).map_err(StorageError::from)
        });
        Ok(Box::pin(messages))
    }
}

fn revision_sort_key(revision: StateRevision) -> String {
    format!("{:020}", revision.get())
}

fn parse_revision_sort_key(value: &str) -> StorageResult<StateRevision> {
    if value.len() != 20 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(StorageError::CorruptData {
            entity: "player_snapshot_revision",
            field: "revision",
        });
    }
    value
        .parse::<u64>()
        .map(StateRevision::new)
        .map_err(|_| StorageError::CorruptData {
            entity: "player_snapshot_revision",
            field: "revision",
        })
}

#[cfg(test)]
mod tests {
    use pepeaudio_core::{GuildId, MAX_PLAYER_SNAPSHOT_JSON_BYTES, StateRevision};

    use super::{SnapshotEvent, decode_snapshot, parse_revision_sort_key, revision_sort_key};

    #[test]
    fn event_uses_string_snowflake_and_numeric_revision() {
        let event = SnapshotEvent {
            guild_id: GuildId::new(42).expect("guild"),
            revision: StateRevision::new(7),
        };
        assert_eq!(
            serde_json::to_string(&event).expect("event JSON"),
            r#"{"guild_id":"42","revision":7}"#
        );
    }

    #[test]
    fn fixed_width_revision_keys_sort_across_f64_precision_boundary() {
        let lower = revision_sort_key(StateRevision::new(9_007_199_254_740_992));
        let higher = revision_sort_key(StateRevision::new(9_007_199_254_740_993));

        assert!(lower < higher);
        assert_eq!(revision_sort_key(StateRevision::new(u64::MAX)).len(), 20);
    }

    #[test]
    fn revision_watermarks_round_trip_and_reject_corruption() {
        let maximum = StateRevision::new(u64::MAX);
        assert_eq!(
            parse_revision_sort_key(&revision_sort_key(maximum)).expect("watermark"),
            maximum
        );
        assert!(parse_revision_sort_key("7").is_err());
        assert!(parse_revision_sort_key("0000000000000000000x").is_err());
    }

    #[test]
    fn snapshot_decoder_rejects_oversized_cache_data_before_json_parsing() {
        let guild = GuildId::new(42).expect("guild");
        let oversized = "x".repeat(MAX_PLAYER_SNAPSHOT_JSON_BYTES + 1);
        let error = decode_snapshot(guild, oversized.as_bytes()).expect_err("oversized snapshot");
        assert!(matches!(
            error,
            crate::StorageError::CorruptData {
                entity: "player_snapshot",
                field: "encoded_size"
            }
        ));
    }
}
