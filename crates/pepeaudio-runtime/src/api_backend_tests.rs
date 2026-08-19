use std::{
    collections::HashMap,
    num::NonZeroU32,
    sync::{Arc, Mutex},
    time::Duration,
};

use async_trait::async_trait;
use futures_util::stream;
use pepeaudio_api::{CommandRouter as _, PlayerEventSource as _, ReadinessProbe as _, RouteError};
use pepeaudio_core::{
    CommandEnvelope, GuildId, PlayerCommand, PlayerSnapshot, PlayerState, RepeatMode,
    StateRevision, UnixTimeMillis, UserId, Volume,
};
use pepeaudio_storage::{
    CommandEnqueue, CommandProducer, SnapshotEvent, SnapshotEventStream, SnapshotEventSubscriber,
    SnapshotStore, SnapshotWrite, StorageError, StorageResult,
};
use tokio::sync::{Notify, mpsc};

use super::ValkeyApiBackend;
use crate::{ApiBackendRuntime, RuntimeError};

#[derive(Clone, Default)]
struct FakeStore {
    inner: Arc<Mutex<FakeStoreInner>>,
    subscription_changed: Arc<Notify>,
}

#[derive(Default)]
struct FakeStoreInner {
    command_rate_limit: Option<Duration>,
    enqueued_commands: usize,
    snapshots: HashMap<GuildId, PlayerSnapshot>,
    corrupt_snapshots: std::collections::HashSet<GuildId>,
    subscriptions: Vec<Option<mpsc::UnboundedSender<StorageResult<SnapshotEvent>>>>,
}

impl FakeStore {
    fn set_snapshot(&self, snapshot: PlayerSnapshot) {
        self.lock().snapshots.insert(snapshot.guild_id, snapshot);
    }

    fn close_subscription(&self, index: usize) {
        self.lock().subscriptions[index].take();
    }

    fn mark_snapshot_corrupt(&self, guild_id: GuildId) {
        self.lock().corrupt_snapshots.insert(guild_id);
    }

    fn send_snapshot_event(&self, index: usize, event: SnapshotEvent) {
        self.lock().subscriptions[index]
            .as_ref()
            .expect("subscription remains open")
            .send(Ok(event))
            .expect("subscription receiver remains open");
    }

    fn set_command_rate_limit(&self, retry_after: Duration) {
        self.lock().command_rate_limit = Some(retry_after);
    }

    fn enqueued_commands(&self) -> usize {
        self.lock().enqueued_commands
    }

    async fn wait_for_subscriptions(&self, count: usize) {
        loop {
            let changed = self.subscription_changed.notified();
            if self.lock().subscriptions.len() >= count {
                return;
            }
            changed.await;
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, FakeStoreInner> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

#[async_trait]
impl SnapshotStore for FakeStore {
    async fn get_snapshot(&self, guild_id: GuildId) -> StorageResult<Option<PlayerSnapshot>> {
        let inner = self.lock();
        if inner.corrupt_snapshots.contains(&guild_id) {
            return Err(StorageError::CorruptData {
                entity: "player_snapshot",
                field: "encoded_size",
            });
        }
        Ok(inner.snapshots.get(&guild_id).cloned())
    }

    async fn get_snapshot_revision(
        &self,
        guild_id: GuildId,
    ) -> StorageResult<Option<StateRevision>> {
        Ok(self
            .lock()
            .snapshots
            .get(&guild_id)
            .map(|snapshot| snapshot.revision))
    }

    async fn invalidate_snapshot(&self, guild_id: GuildId) -> StorageResult<()> {
        self.lock().snapshots.remove(&guild_id);
        Ok(())
    }

    async fn put_snapshot_if_newer(
        &self,
        snapshot: &PlayerSnapshot,
        _ttl: Duration,
    ) -> StorageResult<SnapshotWrite> {
        self.set_snapshot(snapshot.clone());
        Ok(SnapshotWrite::Stored)
    }
}

#[async_trait]
impl SnapshotEventSubscriber for FakeStore {
    async fn subscribe_snapshot_events(&self) -> StorageResult<SnapshotEventStream> {
        let (sender, receiver) = mpsc::unbounded_channel();
        self.lock().subscriptions.push(Some(sender));
        self.subscription_changed.notify_waiters();
        Ok(Box::pin(stream::unfold(receiver, |mut receiver| async {
            receiver.recv().await.map(|item| (item, receiver))
        })))
    }
}

#[async_trait]
impl CommandProducer for FakeStore {
    async fn enqueue_command(
        &self,
        _envelope: &CommandEnvelope,
        _shard_total: u32,
        _result_retention: Duration,
    ) -> StorageResult<CommandEnqueue> {
        let mut inner = self.lock();
        inner.enqueued_commands += 1;
        if let Some(retry_after) = inner.command_rate_limit {
            return Err(StorageError::RateLimited {
                resource: "player command admission",
                retry_after,
            });
        }
        Ok(CommandEnqueue::Enqueued {
            entry_id: "1-0".to_owned(),
        })
    }
}

#[tokio::test]
async fn command_route_preserves_rate_limit_without_leaking_identity() {
    let store = FakeStore::default();
    store.set_command_rate_limit(Duration::from_secs(37));
    let (backend, runtime) = ValkeyApiBackend::start(
        store.clone(),
        NonZeroU32::new(1).expect("shard total"),
        8,
        Duration::from_millis(10),
    )
    .await
    .expect("backend");
    let guild_id = GuildId::new(42).expect("guild ID");
    let envelope = CommandEnvelope::new(
        guild_id,
        Some(UserId::new(7).expect("actor user ID")),
        StateRevision::INITIAL,
        UnixTimeMillis::new(2_000),
        PlayerCommand::Play,
    );

    assert_eq!(
        backend.route(envelope, UnixTimeMillis::new(1_000)).await,
        Err(RouteError::RateLimited {
            retry_after: Duration::from_secs(37)
        })
    );
    assert_eq!(store.enqueued_commands(), 1);
    runtime.shutdown().await.expect("runtime shutdown");
}

#[tokio::test]
async fn command_route_rejects_missing_web_actor_before_storage() {
    let store = FakeStore::default();
    let (backend, runtime) = ValkeyApiBackend::start(
        store.clone(),
        NonZeroU32::new(1).expect("shard total"),
        8,
        Duration::from_millis(10),
    )
    .await
    .expect("backend");
    let envelope = CommandEnvelope::new(
        GuildId::new(42).expect("guild ID"),
        None,
        StateRevision::INITIAL,
        UnixTimeMillis::new(2_000),
        PlayerCommand::Play,
    );

    assert_eq!(
        backend.route(envelope, UnixTimeMillis::new(1_000)).await,
        Err(RouteError::InvalidCommand)
    );
    assert_eq!(store.enqueued_commands(), 0);
    runtime.shutdown().await.expect("runtime shutdown");
}

#[tokio::test(start_paused = true)]
async fn reconnect_republishes_current_state_to_existing_guild_subscribers() {
    let guild_id = GuildId::new(42).expect("guild ID");
    let store = FakeStore::default();
    store.set_snapshot(snapshot(guild_id, 1));
    let (backend, runtime) = ValkeyApiBackend::start(
        store.clone(),
        NonZeroU32::new(1).expect("shard total"),
        8,
        Duration::from_millis(10),
    )
    .await
    .expect("backend");
    let mut events = backend.subscribe(guild_id).expect("event receiver");

    store.set_snapshot(snapshot(guild_id, 3));
    store.close_subscription(0);
    while backend
        .subscription_ready
        .load(std::sync::atomic::Ordering::Acquire)
    {
        tokio::task::yield_now().await;
    }
    tokio::time::advance(Duration::from_millis(10)).await;
    store.wait_for_subscriptions(2).await;

    let event = events.recv().await.expect("republished snapshot");
    assert_eq!(event.snapshot.revision, StateRevision::new(3));
    runtime.shutdown().await.expect("runtime shutdown");
}

#[tokio::test]
async fn corrupt_snapshot_is_isolated_from_other_guild_events() {
    let corrupt_guild = GuildId::new(41).expect("corrupt guild ID");
    let healthy_guild = GuildId::new(42).expect("healthy guild ID");
    let store = FakeStore::default();
    store.mark_snapshot_corrupt(corrupt_guild);
    store.set_snapshot(snapshot(healthy_guild, 2));
    let (backend, runtime) = ValkeyApiBackend::start(
        store.clone(),
        NonZeroU32::new(1).expect("shard total"),
        8,
        Duration::from_millis(10),
    )
    .await
    .expect("backend");
    let mut events = backend.subscribe(healthy_guild).expect("event receiver");

    store.send_snapshot_event(
        0,
        SnapshotEvent {
            guild_id: corrupt_guild,
            revision: StateRevision::new(1),
        },
    );
    store.send_snapshot_event(
        0,
        SnapshotEvent {
            guild_id: healthy_guild,
            revision: StateRevision::new(2),
        },
    );

    let event = tokio::time::timeout(Duration::from_secs(1), events.recv())
        .await
        .expect("healthy guild event remains live")
        .expect("event channel remains open");
    assert_eq!(event.snapshot.guild_id, healthy_guild);
    assert_eq!(store.lock().subscriptions.len(), 1);
    assert!(
        backend
            .subscription_ready
            .load(std::sync::atomic::Ordering::Acquire)
    );
    runtime.shutdown().await.expect("runtime shutdown");
}

#[tokio::test]
async fn corrupt_probe_snapshot_does_not_mask_healthy_valkey_connectivity() {
    let store = FakeStore::default();
    store.mark_snapshot_corrupt(GuildId::new(1).expect("probe guild ID"));
    let (backend, runtime) = ValkeyApiBackend::start(
        store,
        NonZeroU32::new(1).expect("shard total"),
        8,
        Duration::from_millis(10),
    )
    .await
    .expect("backend");

    assert_eq!(backend.ready().await, Ok(()));
    runtime.shutdown().await.expect("runtime shutdown");
}

#[tokio::test]
async fn reconnect_recovery_skips_corrupt_guilds_and_republishes_healthy_ones() {
    let corrupt_guild = GuildId::new(41).expect("corrupt guild ID");
    let healthy_guild = GuildId::new(42).expect("healthy guild ID");
    let store = FakeStore::default();
    store.mark_snapshot_corrupt(corrupt_guild);
    store.set_snapshot(snapshot(healthy_guild, 3));
    let (backend, runtime) = ValkeyApiBackend::start(
        store,
        NonZeroU32::new(1).expect("shard total"),
        8,
        Duration::from_millis(10),
    )
    .await
    .expect("backend");
    let _corrupt_events = backend.subscribe(corrupt_guild).expect("corrupt receiver");
    let mut healthy_events = backend.subscribe(healthy_guild).expect("healthy receiver");

    assert!(backend.publish_current_subscriber_snapshots().await);
    let event = healthy_events
        .recv()
        .await
        .expect("healthy snapshot republished");
    assert_eq!(event.snapshot.guild_id, healthy_guild);
    assert_eq!(event.snapshot.revision, StateRevision::new(3));
    runtime.shutdown().await.expect("runtime shutdown");
}

#[tokio::test]
async fn subscription_task_clean_return_is_a_fatal_runtime_exit() {
    let (shutdown, _receiver) = tokio::sync::watch::channel(false);
    let mut runtime = ApiBackendRuntime {
        shutdown: Some(shutdown),
        task: Some(tokio::spawn(async {})),
    };

    assert!(matches!(
        runtime.wait_for_unexpected_exit().await,
        RuntimeError::RequiredTaskStopped {
            task: "api snapshot subscription"
        }
    ));
    runtime.shutdown().await.expect("already-reaped runtime");
}

#[tokio::test]
async fn subscription_task_panic_preserves_join_failure() {
    let (shutdown, _receiver) = tokio::sync::watch::channel(false);
    let mut runtime = ApiBackendRuntime {
        shutdown: Some(shutdown),
        task: Some(tokio::spawn(async { panic!("subscription panic") })),
    };

    match runtime.wait_for_unexpected_exit().await {
        RuntimeError::Task(error) => assert!(error.is_panic()),
        error => panic!("unexpected error: {error}"),
    }
}

fn snapshot(guild_id: GuildId, revision: u64) -> PlayerSnapshot {
    PlayerSnapshot {
        guild_id,
        voice_channel_id: None,
        revision: StateRevision::new(revision),
        state: PlayerState::Disconnected,
        current_track: None,
        queued_tracks: 0,
        upcoming_tracks: Vec::new(),
        has_previous_track: false,
        volume: Volume::DEFAULT,
        repeat_mode: RepeatMode::Off,
        shuffle_enabled: false,
        hrir_preset: None,
        spatial_audio_enabled: false,
        observed_at: UnixTimeMillis::new(revision),
    }
}
