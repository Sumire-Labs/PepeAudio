use std::time::Duration;

use pepeaudio_core::StateRevision;
use pepeaudio_player::SnapshotPublisher as _;

use super::support::{Behavior, FakeStore, RETRY, guild, runtime, snapshot};

#[tokio::test(start_paused = true)]
async fn failed_write_recovers_without_another_actor_publication() {
    let store = FakeStore::default();
    let guild_id = guild(1);
    store.set_behavior(guild_id, Behavior::Fail);
    let runtime = runtime(store.clone());
    let handle = runtime.handle();
    let mut publisher = handle.publisher(guild_id).await.expect("publisher");

    publisher
        .publish(&snapshot(guild_id, 1))
        .await
        .expect("mailbox accepts immediately");
    store.wait_for_attempts(guild_id, 1).await;
    tokio::task::yield_now().await;
    store.set_behavior(guild_id, Behavior::Succeed);
    tokio::time::advance(RETRY).await;
    store.wait_for_stored(guild_id, 1).await;

    assert_eq!(store.attempts(guild_id), vec![StateRevision::new(1); 2]);
    assert_eq!(store.stored(guild_id), vec![StateRevision::new(1)]);
    runtime.shutdown().await.expect("final flush");
}

#[tokio::test(start_paused = true)]
async fn retry_backoff_doubles_and_remains_bounded() {
    let store = FakeStore::default();
    let guild_id = guild(6);
    store.set_behavior(guild_id, Behavior::Fail);
    let runtime = runtime(store.clone());
    let mut publisher = runtime
        .handle()
        .publisher(guild_id)
        .await
        .expect("publisher");
    publisher
        .publish(&snapshot(guild_id, 1))
        .await
        .expect("publish");
    store.wait_for_attempts(guild_id, 1).await;
    tokio::task::yield_now().await;

    for (before, expected_attempts) in [
        (Duration::from_millis(9), 1),
        (Duration::from_millis(1), 2),
        (Duration::from_millis(19), 2),
        (Duration::from_millis(1), 3),
        (Duration::from_millis(39), 3),
        (Duration::from_millis(1), 4),
        (Duration::from_millis(39), 4),
        (Duration::from_millis(1), 5),
    ] {
        tokio::time::advance(before).await;
        if store.attempts(guild_id).len() < expected_attempts {
            store.wait_for_attempts(guild_id, expected_attempts).await;
        }
        assert_eq!(store.attempts(guild_id).len(), expected_attempts);
        tokio::task::yield_now().await;
    }

    store.set_behavior(guild_id, Behavior::Succeed);
    runtime.shutdown().await.expect("final flush");
}

#[tokio::test(start_paused = true)]
async fn mailbox_coalesces_to_latest_revision_without_rollback() {
    let store = FakeStore::default();
    let guild_id = guild(2);
    store.set_behavior(guild_id, Behavior::Fail);
    let runtime = runtime(store.clone());
    let handle = runtime.handle();
    let mut publisher = handle.publisher(guild_id).await.expect("publisher");

    publisher
        .publish(&snapshot(guild_id, 1))
        .await
        .expect("rev 1");
    store.wait_for_attempts(guild_id, 1).await;
    tokio::task::yield_now().await;
    publisher
        .publish(&snapshot(guild_id, 2))
        .await
        .expect("rev 2");
    publisher
        .publish(&snapshot(guild_id, 3))
        .await
        .expect("rev 3");
    publisher
        .publish(&snapshot(guild_id, 2))
        .await
        .expect("stale actor publication is harmless");
    assert_eq!(
        handle.latest_revision(guild_id).await,
        Some(StateRevision::new(3))
    );
    store.set_behavior(guild_id, Behavior::Succeed);
    tokio::time::advance(RETRY).await;
    store.wait_for_stored(guild_id, 1).await;

    assert_eq!(
        store.attempts(guild_id),
        vec![StateRevision::new(1), StateRevision::new(3)]
    );
    publisher
        .publish(&snapshot(guild_id, 2))
        .await
        .expect("confirmed older revision is ignored");
    tokio::task::yield_now().await;
    assert_eq!(store.attempts(guild_id).len(), 2);
    runtime.shutdown().await.expect("shutdown");
}
