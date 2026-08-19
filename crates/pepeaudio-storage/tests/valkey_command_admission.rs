//! Optional live test for distributed Web command admission.
//!
//! Set `PEPEAUDIO_TEST_VALKEY_URL` to a disposable logical database and run
//! this ignored test explicitly.

use std::time::Duration;

use pepeaudio_core::{
    CommandEnvelope, GuildId, PlayerCommand, PlayerCommandRateLimit, StateRevision, UnixTimeMillis,
    UserId,
};
use pepeaudio_storage::{CommandEnqueue, CommandProducer, Keyspace, StorageError, ValkeyStore};
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires a disposable Valkey endpoint"]
async fn admission_is_atomic_shared_and_bounded() {
    let url = std::env::var("PEPEAUDIO_TEST_VALKEY_URL")
        .expect("set PEPEAUDIO_TEST_VALKEY_URL for the ignored test");
    let prefix = format!("pepeaudio:test:{}", Uuid::new_v4());
    let keyspace = Keyspace::new(&prefix).expect("valid test keyspace");
    let first = ValkeyStore::connect(&url, keyspace.clone())
        .await
        .expect("connect first API replica");
    let second = ValkeyStore::connect(&url, keyspace)
        .await
        .expect("connect second API replica");
    let mut raw = redis::Client::open(url)
        .expect("test Valkey URL")
        .get_connection_manager()
        .await
        .expect("raw test connection");
    let guild_id = GuildId::new(42).expect("guild ID");
    let actor_user_id = UserId::new(7).expect("actor user ID");
    let policy = PlayerCommandRateLimit::STANDARD;

    assert_wrong_key_type_leaves_other_keys_untouched(&first, &mut raw, &prefix).await;
    fill_actor_limit(&first, &second, &mut raw, &prefix, guild_id, actor_user_id).await;
    assert_actor_rejection(&second, &mut raw, &prefix, guild_id, actor_user_id).await;
    fill_remaining_guild_limit(&first, &second, guild_id, policy).await;
    assert_guild_rejection_and_bounds(&first, &mut raw, &prefix, guild_id, policy).await;
}

async fn fill_actor_limit(
    first: &ValkeyStore,
    second: &ValkeyStore,
    connection: &mut redis::aio::ConnectionManager,
    prefix: &str,
    guild_id: GuildId,
    actor_user_id: UserId,
) {
    let policy = PlayerCommandRateLimit::STANDARD;
    for attempt in 0..policy.per_actor_per_guild() {
        let store = if attempt % 2 == 0 { first } else { second };
        let command = envelope(guild_id, actor_user_id);
        store
            .enqueue_command(&command, 1, Duration::from_secs(30))
            .await
            .expect("command within actor limit");
        if attempt == 0 {
            assert_retry_is_free(second, connection, prefix, &command).await;
        }
    }
}

async fn assert_retry_is_free(
    storage: &ValkeyStore,
    connection: &mut redis::aio::ConnectionManager,
    prefix: &str,
    command: &CommandEnvelope,
) {
    let stream = format!("{prefix}:cmd:shard:0");
    let actor = command.actor_user_id.expect("authenticated command actor");
    let actor_key = format!(
        "{prefix}:rate:player-command:guild:{}:actor:{actor}",
        command.guild_id
    );
    let guild_key = format!(
        "{prefix}:rate:player-command:guild:{}:all",
        command.guild_id
    );
    let stream_before = stream_length(connection, &stream).await;
    let actor_before = hash_count(connection, &actor_key).await;
    let guild_before = hash_count(connection, &guild_key).await;

    let retry = storage
        .enqueue_command(command, 1, Duration::from_secs(30))
        .await
        .expect("same command retry is deduplicated");

    assert_eq!(retry, CommandEnqueue::AlreadyPending);
    assert_eq!(hash_count(connection, &actor_key).await, actor_before);
    assert_eq!(hash_count(connection, &guild_key).await, guild_before);
    assert_eq!(stream_length(connection, &stream).await, stream_before);
}

async fn assert_actor_rejection(
    storage: &ValkeyStore,
    connection: &mut redis::aio::ConnectionManager,
    prefix: &str,
    guild_id: GuildId,
    actor_user_id: UserId,
) {
    let actor_key = format!("{prefix}:rate:player-command:guild:{guild_id}:actor:{actor_user_id}");
    let guild_key = format!("{prefix}:rate:player-command:guild:{guild_id}:all");
    let actor_before = hash_count(connection, &actor_key).await;
    let guild_before = hash_count(connection, &guild_key).await;
    let rejected = envelope(guild_id, actor_user_id);
    let result = storage
        .enqueue_command(&rejected, 1, Duration::from_secs(30))
        .await;

    assert_rate_limited(&result);
    assert_eq!(hash_count(connection, &actor_key).await, actor_before);
    assert_eq!(hash_count(connection, &guild_key).await, guild_before);
    assert_absent_result(connection, prefix, &rejected).await;
}

async fn fill_remaining_guild_limit(
    first: &ValkeyStore,
    second: &ValkeyStore,
    guild_id: GuildId,
    policy: PlayerCommandRateLimit,
) {
    let remaining = policy.per_guild() - policy.per_actor_per_guild();
    for offset in 0..remaining {
        let actor = UserId::new(100 + u64::from(offset)).expect("distinct actor user ID");
        let store = if offset % 2 == 0 { second } else { first };
        store
            .enqueue_command(&envelope(guild_id, actor), 1, Duration::from_secs(30))
            .await
            .expect("command within guild limit");
    }
}

async fn assert_guild_rejection_and_bounds(
    storage: &ValkeyStore,
    connection: &mut redis::aio::ConnectionManager,
    prefix: &str,
    guild_id: GuildId,
    policy: PlayerCommandRateLimit,
) {
    let rejected_actor = UserId::new(999).expect("rejected actor user ID");
    let rejected = envelope(guild_id, rejected_actor);
    let result = storage
        .enqueue_command(&rejected, 1, Duration::from_secs(30))
        .await;
    assert_rate_limited(&result);
    assert_absent_result(connection, prefix, &rejected).await;
    let rejected_actor_key =
        format!("{prefix}:rate:player-command:guild:{guild_id}:actor:{rejected_actor}");
    assert_eq!(exists(connection, &rejected_actor_key).await, 0);

    let guild_key = format!("{prefix}:rate:player-command:guild:{guild_id}:all");
    assert_eq!(hash_count(connection, &guild_key).await, policy.per_guild());
    assert_rate_key_bounds(connection, prefix, guild_id, policy).await;
    let stream = format!("{prefix}:cmd:shard:0");
    assert_eq!(
        stream_length(connection, &stream).await,
        usize::try_from(policy.per_guild()).expect("limit fits usize")
    );
}

async fn assert_rate_key_bounds(
    connection: &mut redis::aio::ConnectionManager,
    prefix: &str,
    guild_id: GuildId,
    policy: PlayerCommandRateLimit,
) {
    let rate_keys: Vec<String> = redis::cmd("KEYS")
        .arg(format!("{prefix}:rate:player-command:guild:{guild_id}:*"))
        .query_async(connection)
        .await
        .expect("list isolated rate keys");
    let maximum_rate_keys = usize::try_from(policy.per_guild()).expect("limit fits usize") + 1;
    assert!(rate_keys.len() <= maximum_rate_keys);
    for key in rate_keys {
        let ttl_ms: i64 = redis::cmd("PTTL")
            .arg(key)
            .query_async(connection)
            .await
            .expect("read rate key TTL");
        assert!((1..=60_000).contains(&ttl_ms));
    }
}

async fn assert_wrong_key_type_leaves_other_keys_untouched(
    storage: &ValkeyStore,
    connection: &mut redis::aio::ConnectionManager,
    prefix: &str,
) {
    let guild_id = GuildId::new(84).expect("corrupt-key guild ID");
    let actor_user_id = UserId::new(9).expect("corrupt-key actor ID");
    let command = envelope(guild_id, actor_user_id);
    let actor_key = format!("{prefix}:rate:player-command:guild:{guild_id}:actor:{actor_user_id}");
    let guild_key = format!("{prefix}:rate:player-command:guild:{guild_id}:all");
    let stream_key = format!("{prefix}:cmd:shard:0");
    redis::cmd("SET")
        .arg(&actor_key)
        .arg("wrong-type")
        .query_async::<()>(connection)
        .await
        .expect("inject wrong key type");
    let stream_before: usize = redis::cmd("XLEN")
        .arg(&stream_key)
        .query_async(connection)
        .await
        .expect("stream before wrong key type");

    assert!(matches!(
        storage
            .enqueue_command(&command, 1, Duration::from_secs(30))
            .await,
        Err(StorageError::CorruptData {
            entity: "command admission",
            field: "key_type"
        })
    ));
    assert_absent_result(connection, prefix, &command).await;
    assert_eq!(exists(connection, &guild_key).await, 0);
    let stream_after: usize = redis::cmd("XLEN")
        .arg(stream_key)
        .query_async(connection)
        .await
        .expect("stream after wrong key type");
    assert_eq!(stream_after, stream_before);
}

fn envelope(guild_id: GuildId, actor_user_id: UserId) -> CommandEnvelope {
    CommandEnvelope::new(
        guild_id,
        Some(actor_user_id),
        StateRevision::INITIAL,
        UnixTimeMillis::new(10_000),
        PlayerCommand::Play,
    )
}

fn assert_rate_limited(result: &Result<CommandEnqueue, StorageError>) {
    assert!(matches!(
        result,
        Err(StorageError::RateLimited {
            retry_after,
            ..
        }) if (Duration::from_secs(1)..=Duration::from_mins(1)).contains(retry_after)
    ));
}

async fn stream_length(connection: &mut redis::aio::ConnectionManager, key: &str) -> usize {
    redis::cmd("XLEN")
        .arg(key)
        .query_async(connection)
        .await
        .expect("read command stream length")
}

async fn hash_count(connection: &mut redis::aio::ConnectionManager, key: &str) -> u32 {
    redis::cmd("HGET")
        .arg(key)
        .arg("count")
        .query_async(connection)
        .await
        .expect("read admission count")
}

async fn exists(connection: &mut redis::aio::ConnectionManager, key: &str) -> usize {
    redis::cmd("EXISTS")
        .arg(key)
        .query_async(connection)
        .await
        .expect("read key existence")
}

async fn assert_absent_result(
    connection: &mut redis::aio::ConnectionManager,
    prefix: &str,
    envelope: &CommandEnvelope,
) {
    let key = format!(
        "{prefix}:cmd-result:{}:{}",
        envelope.guild_id, envelope.command_id
    );
    assert_eq!(exists(connection, &key).await, 0);
}
