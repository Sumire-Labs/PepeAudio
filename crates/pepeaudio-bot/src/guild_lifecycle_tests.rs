use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

use async_trait::async_trait;
use pepeaudio_core::GuildId;
use pepeaudio_runtime::GuildPresenceRuntime;
use pepeaudio_storage::{BotPresenceStore, StorageResult};
use tokio::sync::{Mutex, Notify};

use super::{GuildLifecycleError, GuildLifecycleRuntime, RetryPolicy, SnapshotInvalidator};

#[derive(Clone, Default)]
struct FakePresenceStore {
    owners: Arc<Mutex<HashMap<GuildId, String>>>,
    events: Arc<Mutex<Vec<&'static str>>>,
}

#[async_trait]
impl BotPresenceStore for FakePresenceStore {
    async fn refresh_bot_presence(
        &self,
        guild_id: GuildId,
        instance_id: &str,
        _: Duration,
    ) -> StorageResult<()> {
        self.events.lock().await.push("present");
        self.owners
            .lock()
            .await
            .insert(guild_id, instance_id.to_owned());
        Ok(())
    }

    async fn clear_bot_presence(
        &self,
        guild_id: GuildId,
        instance_id: &str,
    ) -> StorageResult<bool> {
        self.events.lock().await.push("absent");
        let mut owners = self.owners.lock().await;
        let owned = owners
            .get(&guild_id)
            .is_some_and(|owner| owner == instance_id);
        if owned {
            owners.remove(&guild_id);
        }
        Ok(owned)
    }

    async fn is_bot_present(&self, guild_id: GuildId) -> StorageResult<bool> {
        Ok(self.owners.lock().await.contains_key(&guild_id))
    }
}

struct FailingSnapshots {
    failures: AtomicUsize,
    calls: AtomicUsize,
    events: Arc<Mutex<Vec<&'static str>>>,
}

impl FailingSnapshots {
    fn new(failures: usize, events: Arc<Mutex<Vec<&'static str>>>) -> Self {
        Self {
            failures: AtomicUsize::new(failures),
            calls: AtomicUsize::new(0),
            events,
        }
    }
}

#[async_trait]
impl SnapshotInvalidator for FailingSnapshots {
    async fn invalidate(&self, _: GuildId) -> Result<(), GuildLifecycleError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self
            .failures
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |remaining| {
                remaining.checked_sub(1)
            })
            .is_ok()
        {
            self.events.lock().await.push("invalidate_failed");
            Err(GuildLifecycleError)
        } else {
            self.events.lock().await.push("invalidate_succeeded");
            Ok(())
        }
    }
}

struct BlockingSnapshots {
    first: AtomicBool,
    started: Notify,
    release: Notify,
    events: Arc<Mutex<Vec<&'static str>>>,
}

#[async_trait]
impl SnapshotInvalidator for BlockingSnapshots {
    async fn invalidate(&self, _: GuildId) -> Result<(), GuildLifecycleError> {
        if self.first.swap(false, Ordering::SeqCst) {
            self.events.lock().await.push("invalidate_started");
            self.started.notify_one();
            self.release.notified().await;
        }
        self.events.lock().await.push("invalidate_succeeded");
        Ok(())
    }
}

fn retry_policy() -> RetryPolicy {
    RetryPolicy {
        short_delays: [Duration::ZERO; 2],
        background_interval: Duration::from_secs(1),
    }
}

fn start_presence(store: FakePresenceStore) -> GuildPresenceRuntime {
    GuildPresenceRuntime::start(
        store,
        "bot-test".into(),
        Duration::from_secs(30),
        Duration::from_secs(10),
    )
    .expect("presence runtime")
}

async fn settle_tasks() {
    for _ in 0..8 {
        tokio::task::yield_now().await;
    }
}

#[tokio::test(start_paused = true)]
async fn invalidation_failure_stays_fail_closed_then_repairs_in_background() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let snapshots = Arc::new(FailingSnapshots::new(3, Arc::clone(&events)));
    let presence_store = FakePresenceStore {
        events: Arc::clone(&events),
        ..FakePresenceStore::default()
    };
    let presence = start_presence(presence_store.clone());
    let lifecycle = GuildLifecycleRuntime::start_with(snapshots, presence.handle(), retry_policy());
    let guild_id = GuildId::new(41).expect("guild");

    assert!(
        lifecycle
            .handle()
            .present_on_shard(0, guild_id)
            .await
            .is_err()
    );
    assert!(
        !presence_store
            .is_bot_present(guild_id)
            .await
            .expect("lookup")
    );
    assert!(!events.lock().await.contains(&"present"));

    tokio::time::advance(Duration::from_secs(1)).await;
    settle_tasks().await;
    assert!(
        presence_store
            .is_bot_present(guild_id)
            .await
            .expect("lookup")
    );
    let events = events.lock().await.clone();
    let invalidated = events
        .iter()
        .position(|event| *event == "invalidate_succeeded")
        .expect("successful invalidation");
    let advertised = events
        .iter()
        .position(|event| *event == "present")
        .expect("presence advertisement");
    assert!(invalidated < advertised);

    lifecycle.shutdown().await.expect("lifecycle shutdown");
    presence.shutdown().await.expect("presence shutdown");
}

#[tokio::test(start_paused = true)]
async fn deletion_cancels_a_failed_background_acquisition() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let snapshots = Arc::new(FailingSnapshots::new(100, events));
    let presence_store = FakePresenceStore::default();
    let presence = start_presence(presence_store.clone());
    let lifecycle =
        GuildLifecycleRuntime::start_with(snapshots.clone(), presence.handle(), retry_policy());
    let guild_id = GuildId::new(42).expect("guild");

    assert!(
        lifecycle
            .handle()
            .present_on_shard(0, guild_id)
            .await
            .is_err()
    );
    let absence = lifecycle
        .handle()
        .remove_from_shard(0, guild_id)
        .await
        .expect("absence response");
    assert!(absence.no_longer_owned);
    assert!(absence.update.is_ok());
    let calls_after_delete = snapshots.calls.load(Ordering::SeqCst);

    tokio::time::advance(Duration::from_secs(5)).await;
    settle_tasks().await;
    assert_eq!(snapshots.calls.load(Ordering::SeqCst), calls_after_delete);
    assert!(
        !presence_store
            .is_bot_present(guild_id)
            .await
            .expect("lookup")
    );

    lifecycle.shutdown().await.expect("lifecycle shutdown");
    presence.shutdown().await.expect("presence shutdown");
}

#[tokio::test]
async fn queued_delete_wins_over_an_older_blocked_create() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let snapshots = Arc::new(BlockingSnapshots {
        first: AtomicBool::new(true),
        started: Notify::new(),
        release: Notify::new(),
        events: Arc::clone(&events),
    });
    let presence_store = FakePresenceStore {
        events: Arc::clone(&events),
        ..FakePresenceStore::default()
    };
    let presence = start_presence(presence_store.clone());
    let lifecycle =
        GuildLifecycleRuntime::start_with(snapshots.clone(), presence.handle(), retry_policy());
    let guild_id = GuildId::new(43).expect("guild");

    let create_handle = lifecycle.handle();
    let create = tokio::spawn(async move { create_handle.present_on_shard(0, guild_id).await });
    snapshots.started.notified().await;
    let delete_handle = lifecycle.handle();
    let delete = tokio::spawn(async move {
        delete_handle
            .remove_from_shard(0, guild_id)
            .await
            .expect("absence response")
    });
    settle_tasks().await;
    snapshots.release.notify_one();

    create.await.expect("create task").expect("create update");
    let absence = delete.await.expect("delete task");
    assert!(absence.no_longer_owned);
    assert!(absence.update.is_ok());
    assert!(
        !presence_store
            .is_bot_present(guild_id)
            .await
            .expect("lookup")
    );
    let events = events.lock().await.clone();
    assert_eq!(
        events,
        vec![
            "invalidate_started",
            "invalidate_succeeded",
            "present",
            "absent"
        ]
    );

    lifecycle.shutdown().await.expect("lifecycle shutdown");
    presence.shutdown().await.expect("presence shutdown");
}

#[tokio::test]
async fn ready_reconciliation_replaces_only_the_reported_shard() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let snapshots = Arc::new(FailingSnapshots::new(0, events));
    let presence_store = FakePresenceStore::default();
    let presence = start_presence(presence_store.clone());
    let lifecycle = GuildLifecycleRuntime::start_with(snapshots, presence.handle(), retry_policy());
    let first = GuildId::new(51).expect("guild");
    let second = GuildId::new(52).expect("guild");
    let replacement = GuildId::new(53).expect("guild");
    let handle = lifecycle.handle();
    handle.present_on_shard(0, first).await.expect("first");
    handle.present_on_shard(1, second).await.expect("second");

    let outcome = handle
        .reconcile_shard(0, HashSet::from([replacement]))
        .await
        .expect("reconciliation");
    assert!(outcome.update.is_ok());
    assert_eq!(outcome.removed, vec![first]);
    assert_eq!(
        handle
            .owned_on_shard(0)
            .await
            .into_iter()
            .collect::<HashSet<_>>(),
        HashSet::from([replacement])
    );
    assert_eq!(
        handle
            .owned_on_shard(1)
            .await
            .into_iter()
            .collect::<HashSet<_>>(),
        HashSet::from([second])
    );
    assert!(
        !presence_store
            .is_bot_present(first)
            .await
            .expect("first lookup")
    );
    assert!(
        presence_store
            .is_bot_present(second)
            .await
            .expect("second lookup")
    );
    assert!(
        presence_store
            .is_bot_present(replacement)
            .await
            .expect("replacement lookup")
    );

    lifecycle.shutdown().await.expect("lifecycle shutdown");
    presence.shutdown().await.expect("presence shutdown");
}

#[tokio::test]
async fn one_shard_departure_keeps_another_shards_ownership() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let snapshots = Arc::new(FailingSnapshots::new(0, events));
    let presence_store = FakePresenceStore::default();
    let presence = start_presence(presence_store.clone());
    let lifecycle = GuildLifecycleRuntime::start_with(snapshots, presence.handle(), retry_policy());
    let guild_id = GuildId::new(61).expect("guild");
    let handle = lifecycle.handle();
    handle.present_on_shard(0, guild_id).await.expect("shard 0");
    handle.present_on_shard(1, guild_id).await.expect("shard 1");

    let first_departure = handle
        .remove_from_shard(0, guild_id)
        .await
        .expect("first departure");
    assert!(!first_departure.no_longer_owned);
    assert!(first_departure.update.is_ok());
    assert!(
        presence_store
            .is_bot_present(guild_id)
            .await
            .expect("lookup")
    );

    let final_departure = handle
        .remove_from_shard(1, guild_id)
        .await
        .expect("final departure");
    assert!(final_departure.no_longer_owned);
    assert!(final_departure.update.is_ok());
    assert!(
        !presence_store
            .is_bot_present(guild_id)
            .await
            .expect("lookup")
    );

    lifecycle.shutdown().await.expect("lifecycle shutdown");
    presence.shutdown().await.expect("presence shutdown");
}
