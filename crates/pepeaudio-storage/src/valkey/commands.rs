use std::time::Duration;

use async_trait::async_trait;
use pepeaudio_core::CommandEnvelope;
use redis::{
    AsyncCommands,
    aio::ConnectionManager,
    streams::{StreamAutoClaimOptions, StreamAutoClaimReply, StreamId, StreamReadOptions},
};

use super::ValkeyStore;
use crate::{StorageError, StorageResult};

const ACK_AND_DELETE_SCRIPT: &str = r"
local acknowledged = redis.call('XACK', KEYS[1], ARGV[1], ARGV[2])
if acknowledged == 1 then
  redis.call('XDEL', KEYS[1], ARGV[2])
end
return acknowledged
";

/// A command paired with the Valkey stream entry needed for acknowledgement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReceivedCommand {
    pub entry_id: String,
    pub envelope: CommandEnvelope,
}

/// Result of scanning abandoned pending commands with `XAUTOCLAIM`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaimBatch {
    /// Cursor for the next scan; `0-0` denotes a completed pass.
    pub next_start: String,
    pub commands: Vec<ReceivedCommand>,
    /// Pending IDs whose stream entries had already been trimmed or deleted.
    pub deleted_entry_ids: Vec<String>,
}

/// Shard command consumer used by Bot workers.
#[async_trait]
pub trait CommandConsumer: Send + Sync {
    /// Creates a consumer group at the beginning of the stream when absent.
    async fn ensure_command_group(&self, shard_id: u32, group: &str) -> StorageResult<()>;

    /// Reads never-delivered entries for one consumer. Empty results are normal.
    async fn read_commands(
        &self,
        shard_id: u32,
        group: &str,
        consumer: &str,
        count: usize,
        block: Duration,
    ) -> StorageResult<Vec<ReceivedCommand>>;

    /// Moves sufficiently idle pending entries to this consumer.
    async fn claim_abandoned_commands(
        &self,
        shard_id: u32,
        group: &str,
        consumer: &str,
        minimum_idle: Duration,
        start: &str,
        count: usize,
    ) -> StorageResult<ClaimBatch>;

    /// Acknowledges a successfully applied or deliberately rejected command.
    async fn acknowledge_command(
        &self,
        shard_id: u32,
        group: &str,
        entry_id: &str,
    ) -> StorageResult<bool>;
}

#[async_trait]
impl CommandConsumer for ValkeyStore {
    async fn ensure_command_group(&self, shard_id: u32, group: &str) -> StorageResult<()> {
        validate_consumer_name("consumer group", group)?;
        let mut connection = self.connection.clone();
        let result: redis::RedisResult<()> = connection
            .xgroup_create_mkstream(self.keyspace.command_stream(shard_id), group, "0")
            .await;
        match result {
            Ok(()) => Ok(()),
            Err(error) if error.code() == Some("BUSYGROUP") => Ok(()),
            Err(error) => Err(StorageError::Valkey(error)),
        }
    }

    async fn read_commands(
        &self,
        shard_id: u32,
        group: &str,
        consumer: &str,
        count: usize,
        block: Duration,
    ) -> StorageResult<Vec<ReceivedCommand>> {
        validate_consumer_name("consumer group", group)?;
        validate_consumer_name("consumer", consumer)?;
        let count = count.max(1);
        let block_ms = duration_millis(block, "command stream block")?;
        let options = StreamReadOptions::default()
            .group(group, consumer)
            .count(count)
            .block(block_ms);
        let stream = self.keyspace.command_stream(shard_id);
        let mut connection = self.blocking_connection(shard_id).await?;
        let reply: Option<redis::streams::StreamReadReply> = connection
            .xread_options(&[stream.as_str()], &[">"], &options)
            .await?;
        if let Some(reply) = reply {
            let ids = reply.keys.into_iter().flat_map(|key| key.ids).collect();
            decode_or_discard(&mut connection, &stream, group, ids).await
        } else {
            Ok(Vec::new())
        }
    }

    async fn claim_abandoned_commands(
        &self,
        shard_id: u32,
        group: &str,
        consumer: &str,
        minimum_idle: Duration,
        start: &str,
        count: usize,
    ) -> StorageResult<ClaimBatch> {
        validate_consumer_name("consumer group", group)?;
        validate_consumer_name("consumer", consumer)?;
        validate_stream_id(start)?;
        let idle_ms = duration_millis(minimum_idle, "command claim idle")?;
        let options = StreamAutoClaimOptions::default().count(count.max(1));
        let mut connection = self.connection.clone();
        let stream = self.keyspace.command_stream(shard_id);
        let reply: StreamAutoClaimReply = connection
            .xautoclaim_options(&stream, group, consumer, idle_ms, start, options)
            .await?;
        let commands = decode_or_discard(&mut connection, &stream, group, reply.claimed).await?;
        Ok(ClaimBatch {
            next_start: reply.next_stream_id,
            commands,
            deleted_entry_ids: reply.deleted_ids,
        })
    }

    async fn acknowledge_command(
        &self,
        shard_id: u32,
        group: &str,
        entry_id: &str,
    ) -> StorageResult<bool> {
        validate_consumer_name("consumer group", group)?;
        validate_stream_id(entry_id)?;
        let mut connection = self.connection.clone();
        let stream = self.keyspace.command_stream(shard_id);
        let acknowledged =
            acknowledge_and_delete(&mut connection, &stream, group, entry_id).await?;
        Ok(acknowledged == 1)
    }
}

async fn decode_or_discard(
    connection: &mut ConnectionManager,
    stream: &str,
    group: &str,
    ids: Vec<StreamId>,
) -> StorageResult<Vec<ReceivedCommand>> {
    let mut commands = Vec::with_capacity(ids.len());
    for stream_id in ids {
        let entry_id = stream_id.id.clone();
        if let Ok(command) = decode_stream_id(stream_id) {
            commands.push(command);
            continue;
        }
        // A malformed entry came from Valkey, not from a caller. Remove it
        // from the PEL and stream so it cannot poison this shard's entire
        // batch forever. Its raw payload is intentionally never surfaced.
        acknowledge_and_delete(connection, stream, group, &entry_id).await?;
    }
    Ok(commands)
}

async fn acknowledge_and_delete(
    connection: &mut ConnectionManager,
    stream: &str,
    group: &str,
    entry_id: &str,
) -> StorageResult<usize> {
    redis::Script::new(ACK_AND_DELETE_SCRIPT)
        .key(stream)
        .arg(group)
        .arg(entry_id)
        .invoke_async(connection)
        .await
        .map_err(StorageError::from)
}

fn decode_stream_id(stream_id: StreamId) -> StorageResult<ReceivedCommand> {
    let encoded = stream_id
        .get::<String>("envelope")
        .ok_or(StorageError::CorruptData {
            entity: "shard_command",
            field: "envelope",
        })?;
    Ok(ReceivedCommand {
        entry_id: stream_id.id,
        envelope: serde_json::from_str(&encoded)?,
    })
}

fn duration_millis(duration: Duration, operation: &'static str) -> StorageResult<usize> {
    usize::try_from(duration.as_millis())
        .map_err(|_| StorageError::DurationOutOfRange { operation })
}

fn validate_consumer_name(kind: &'static str, value: &str) -> StorageResult<()> {
    if value.is_empty() || value.len() > 128 || value.chars().any(char::is_control) {
        Err(StorageError::InvalidIdentifier {
            kind,
            reason: "must be 1-128 characters without control characters",
        })
    } else {
        Ok(())
    }
}

fn validate_stream_id(value: &str) -> StorageResult<()> {
    let valid = value == "0"
        || value == "0-0"
        || value.split_once('-').is_some_and(|(left, right)| {
            !left.is_empty()
                && !right.is_empty()
                && left.bytes().all(|byte| byte.is_ascii_digit())
                && right.bytes().all(|byte| byte.is_ascii_digit())
        });
    if valid {
        Ok(())
    } else {
        Err(StorageError::InvalidIdentifier {
            kind: "stream entry ID",
            reason: "must be zero or a numeric milliseconds-sequence pair",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{validate_consumer_name, validate_stream_id};

    #[test]
    fn validates_stream_and_consumer_identifiers_before_commands() {
        assert!(validate_consumer_name("consumer", "bot-local-0").is_ok());
        assert!(validate_consumer_name("consumer", "bad\nname").is_err());
        assert!(validate_stream_id("0-0").is_ok());
        assert!(validate_stream_id("123456-7").is_ok());
        assert!(validate_stream_id("123456-* $ injection").is_err());
    }
}
