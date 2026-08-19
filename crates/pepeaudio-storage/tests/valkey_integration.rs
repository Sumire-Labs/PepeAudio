//! Optional live Valkey snapshot, command, and idempotency smoke test.
//!
//! Set `PEPEAUDIO_TEST_VALKEY_URL` to a disposable logical database and run
//! this ignored test explicitly.

use std::time::Duration;

use pepeaudio_core::{
    CommandEnvelope, CommandResult, CommandResultStatus, GuildId, PlayerCommand, PlayerSnapshot,
    PlayerState, RepeatMode, StateRevision, UnixTimeMillis, UserId, Volume,
};
use pepeaudio_storage::{
    BotPresenceStore, CommandCompletion, CommandConsumer, CommandProducer, CommandResultStore,
    DedupeClaim, IdempotencyStore, Keyspace, SnapshotStore, SnapshotWrite, ValkeyStore,
};
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires a disposable Valkey endpoint"]
async fn round_trips_snapshot_stream_and_idempotency_state() {
    let url = std::env::var("PEPEAUDIO_TEST_VALKEY_URL")
        .expect("set PEPEAUDIO_TEST_VALKEY_URL for the ignored test");
    let prefix = format!("pepeaudio:test:{}", Uuid::new_v4());
    let keyspace = Keyspace::new(&prefix).expect("valid test keyspace");
    let storage = ValkeyStore::connect(&url, keyspace)
        .await
        .expect("connect to Valkey");
    storage.ping().await.expect("PONG from Valkey");
    assert!(!format!("{storage:?}").contains(&url));

    let guild_id = GuildId::new(42).expect("guild");
    test_snapshots(&storage, guild_id, &url, &prefix).await;
    test_idempotency(&storage, guild_id).await;
    test_command_stream(&storage, guild_id, &url, &prefix).await;
    test_bot_presence(&storage, guild_id).await;
}

async fn test_bot_presence(storage: &ValkeyStore, guild_id: GuildId) {
    storage
        .refresh_bot_presence(guild_id, "owner-1", Duration::from_secs(30))
        .await
        .expect("advertise bot presence");
    assert!(storage.is_bot_present(guild_id).await.expect("presence"));
    assert!(
        !storage
            .clear_bot_presence(guild_id, "different-owner")
            .await
            .expect("foreign owner cannot clear")
    );
    assert!(
        storage
            .is_bot_present(guild_id)
            .await
            .expect("still present")
    );
    assert!(
        storage
            .clear_bot_presence(guild_id, "owner-1")
            .await
            .expect("owner clears")
    );
    assert!(!storage.is_bot_present(guild_id).await.expect("absent"));
}

async fn test_snapshots(
    storage: &ValkeyStore,
    guild_id: GuildId,
    valkey_url: &str,
    keyspace: &str,
) {
    let snapshot = snapshot(guild_id, 1);
    assert_eq!(
        storage
            .put_snapshot_if_newer(&snapshot, Duration::from_mins(1))
            .await
            .expect("put snapshot"),
        SnapshotWrite::Stored
    );
    assert_eq!(
        storage
            .put_snapshot_if_newer(&snapshot, Duration::from_mins(1))
            .await
            .expect("reject stale snapshot"),
        SnapshotWrite::Stale
    );
    assert_eq!(
        storage.get_snapshot(guild_id).await.expect("get snapshot"),
        Some(snapshot.clone())
    );
    assert_eq!(
        storage
            .get_snapshot_revision(guild_id)
            .await
            .expect("get revision watermark"),
        Some(snapshot.revision)
    );
    storage
        .invalidate_snapshot(guild_id)
        .await
        .expect("invalidate snapshot body");
    assert_eq!(
        storage.get_snapshot(guild_id).await.expect("body removed"),
        None
    );
    assert_eq!(
        storage
            .get_snapshot_revision(guild_id)
            .await
            .expect("watermark retained"),
        Some(snapshot.revision)
    );

    let mut raw = redis::Client::open(valkey_url)
        .expect("test Valkey URL")
        .get_connection_manager()
        .await
        .expect("raw test connection");
    let snapshot_key = format!("{keyspace}:player:{guild_id}:snapshot");
    let _: () = redis::cmd("SET")
        .arg(&snapshot_key)
        .arg(&[0xff_u8, 0xfe])
        .query_async(&mut raw)
        .await
        .expect("inject non-UTF-8 snapshot");
    assert!(matches!(
        storage.get_snapshot(guild_id).await,
        Err(pepeaudio_storage::StorageError::Json(_))
    ));
    let _: usize = redis::cmd("DEL")
        .arg(&snapshot_key)
        .query_async(&mut raw)
        .await
        .expect("remove non-UTF-8 snapshot");
    let _: usize = redis::cmd("RPUSH")
        .arg(&snapshot_key)
        .arg("wrong-type")
        .query_async(&mut raw)
        .await
        .expect("inject wrong-type snapshot");
    assert!(matches!(
        storage.get_snapshot(guild_id).await,
        Err(pepeaudio_storage::StorageError::CorruptData {
            entity: "player_snapshot",
            field: "key_type"
        })
    ));
    let _: usize = redis::cmd("DEL")
        .arg(snapshot_key)
        .query_async(&mut raw)
        .await
        .expect("remove wrong-type snapshot");
}

async fn test_idempotency(storage: &ValkeyStore, guild_id: GuildId) {
    let idempotency_key = Uuid::new_v4();
    let first_lease = Uuid::new_v4();
    let second_lease = Uuid::new_v4();
    assert_eq!(
        storage
            .claim_idempotency(
                guild_id,
                idempotency_key,
                first_lease,
                Duration::from_secs(10),
            )
            .await
            .expect("claim lease"),
        DedupeClaim::Acquired
    );
    assert_eq!(
        storage
            .claim_idempotency(
                guild_id,
                idempotency_key,
                second_lease,
                Duration::from_secs(10),
            )
            .await
            .expect("observe lease"),
        DedupeClaim::InProgress
    );
    assert!(
        storage
            .complete_idempotency(
                guild_id,
                idempotency_key,
                first_lease,
                Duration::from_mins(1),
            )
            .await
            .expect("complete lease")
    );
    assert_eq!(
        storage
            .claim_idempotency(
                guild_id,
                idempotency_key,
                second_lease,
                Duration::from_secs(10),
            )
            .await
            .expect("observe completion"),
        DedupeClaim::Completed
    );
}

async fn test_command_stream(
    storage: &ValkeyStore,
    guild_id: GuildId,
    valkey_url: &str,
    keyspace: &str,
) {
    storage
        .ensure_command_group(0, "integration")
        .await
        .expect("create command group");
    let mut raw = redis::Client::open(valkey_url)
        .expect("test Valkey URL")
        .get_connection_manager()
        .await
        .expect("raw test connection");
    let stream = format!("{keyspace}:cmd:shard:0");
    let _poison_id: String = redis::cmd("XADD")
        .arg(&stream)
        .arg("*")
        .arg("envelope")
        .arg("{not-json")
        .query_async(&mut raw)
        .await
        .expect("inject a malformed entry");
    let envelope = CommandEnvelope::new(
        guild_id,
        Some(UserId::new(7).expect("actor user ID")),
        StateRevision::new(1),
        UnixTimeMillis::new(10_000),
        PlayerCommand::Play,
    );
    storage
        .enqueue_command(&envelope, 1, Duration::from_secs(30))
        .await
        .expect("enqueue command");
    assert_eq!(
        storage
            .get_command_result(guild_id, envelope.command_id)
            .await
            .expect("pending command result")
            .expect("result exists")
            .status,
        CommandResultStatus::Pending
    );
    let received = storage
        .read_commands(0, "integration", "worker-1", 10, Duration::from_millis(100))
        .await
        .expect("read command");
    assert_eq!(received.len(), 1);
    assert_eq!(received[0].envelope, envelope);
    let lease_token = Uuid::new_v4();
    assert_eq!(
        storage
            .claim_idempotency(
                guild_id,
                envelope.idempotency_key,
                lease_token,
                Duration::from_secs(10),
            )
            .await
            .expect("claim command lease"),
        DedupeClaim::Acquired
    );
    let applied = CommandResult::applied(envelope.command_id, guild_id, StateRevision::new(2));
    assert_eq!(
        storage
            .complete_command(
                envelope.idempotency_key,
                lease_token,
                &applied,
                Duration::from_secs(30),
                Duration::from_secs(30),
            )
            .await
            .expect("complete command and result"),
        CommandCompletion::Stored
    );
    assert_eq!(
        storage
            .get_command_result(guild_id, envelope.command_id)
            .await
            .expect("terminal command result"),
        Some(applied)
    );
    assert!(
        storage
            .acknowledge_command(0, "integration", &received[0].entry_id)
            .await
            .expect("ack command")
    );
    let remaining: usize = redis::cmd("XLEN")
        .arg(stream)
        .query_async(&mut raw)
        .await
        .expect("read stream length");
    assert_eq!(remaining, 0, "poison and valid entries were deleted");
}

fn snapshot(guild_id: GuildId, revision: u64) -> PlayerSnapshot {
    PlayerSnapshot {
        guild_id,
        voice_channel_id: None,
        revision: StateRevision::new(revision),
        state: PlayerState::Paused,
        current_track: None,
        queued_tracks: 0,
        upcoming_tracks: Vec::new(),
        has_previous_track: false,
        volume: Volume::DEFAULT,
        repeat_mode: RepeatMode::Off,
        shuffle_enabled: false,
        hrir_preset: None,
        spatial_audio_enabled: false,
        observed_at: UnixTimeMillis::new(1_000),
    }
}
