use std::time::Duration;

use pepeaudio_core::StateRevision;
use pepeaudio_player::SnapshotPublisher as _;
use pepeaudio_storage::SnapshotStore as _;

use crate::{SnapshotPublishError, SnapshotSupervisorError};

use super::support::{Behavior, FakeStore, WRITE_TIMEOUT, guild, runtime, snapshot};

#[tokio::test(start_paused = true)]
async fn blocked_guild_isolated_and_shutdown_bounds_its_final_write() {
    let store = FakeStore::default();
    let blocked = guild(3);
    let healthy = guild(4);
    store.set_behavior(blocked, Behavior::Hang);
    let runtime = runtime(store.clone());
    let handle = runtime.handle();
    let mut blocked_publisher = handle.publisher(blocked).await.expect("blocked publisher");
    let mut healthy_publisher = handle.publisher(healthy).await.expect("healthy publisher");

    blocked_publisher
        .publish(&snapshot(blocked, 1))
        .await
        .expect("blocked mailbox");
    store.wait_for_attempts(blocked, 1).await;
    healthy_publisher
        .publish(&snapshot(healthy, 7))
        .await
        .expect("healthy mailbox");
    store.wait_for_stored(healthy, 1).await;

    assert_eq!(store.stored(healthy), vec![StateRevision::new(7)]);
    assert!(store.stored(blocked).is_empty());
    let shutdown = tokio::spawn(runtime.shutdown());
    store.wait_for_attempts(blocked, 2).await;
    assert!(!shutdown.is_finished());
    assert_eq!(
        blocked_publisher.publish(&snapshot(blocked, 2)).await,
        Err(SnapshotPublishError::SupervisorClosed)
    );
    assert!(matches!(
        handle.publisher(guild(9)).await,
        Err(SnapshotPublishError::SupervisorClosed)
    ));
    tokio::time::advance(WRITE_TIMEOUT).await;
    assert_eq!(
        shutdown.await.expect("shutdown task"),
        Err(SnapshotSupervisorError::FinalFlushTimedOut)
    );
    assert_eq!(store.attempts(blocked), vec![StateRevision::new(1); 2]);
}

#[tokio::test(start_paused = true)]
async fn cancelling_shutdown_aborts_drained_snapshot_workers() {
    let store = FakeStore::default();
    let blocked = guild(8);
    store.set_behavior(blocked, Behavior::Hang);
    let runtime = runtime(store.clone());
    let mut publisher = runtime
        .handle()
        .publisher(blocked)
        .await
        .expect("publisher");
    publisher
        .publish(&snapshot(blocked, 1))
        .await
        .expect("blocked mailbox");
    store.wait_for_attempts(blocked, 1).await;

    let shutdown = tokio::spawn(runtime.shutdown());
    store.wait_for_attempts(blocked, 2).await;
    shutdown.abort();
    assert!(
        shutdown
            .await
            .expect_err("shutdown was cancelled")
            .is_cancelled()
    );

    store.set_behavior(blocked, Behavior::Succeed);
    tokio::time::advance(Duration::from_mins(1)).await;
    tokio::task::yield_now().await;
    assert_eq!(store.attempts(blocked), vec![StateRevision::new(1); 2]);
}

#[tokio::test(start_paused = true)]
async fn shutdown_reports_a_failed_final_write_and_closes_publishers() {
    let store = FakeStore::default();
    let guild_id = guild(5);
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

    assert_eq!(
        runtime.shutdown().await,
        Err(SnapshotSupervisorError::FinalFlushFailed)
    );
    assert_eq!(
        publisher.publish(&snapshot(guild_id, 2)).await,
        Err(SnapshotPublishError::SupervisorClosed)
    );
    assert_eq!(store.attempts(guild_id), vec![StateRevision::new(1); 2]);
}

#[tokio::test(start_paused = true)]
async fn shutdown_flushes_the_latest_mailbox_revision_to_the_durable_watermark() {
    let store = FakeStore::default();
    let guild_id = guild(7);
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
        .expect("first revision");
    store.wait_for_attempts(guild_id, 1).await;
    publisher
        .publish(&snapshot(guild_id, 2))
        .await
        .expect("coalesced revision");
    publisher
        .publish(&snapshot(guild_id, 3))
        .await
        .expect("latest revision");

    store.set_behavior(guild_id, Behavior::Succeed);
    runtime.shutdown().await.expect("final flush");

    assert_eq!(
        store.attempts(guild_id),
        vec![StateRevision::new(1), StateRevision::new(3)]
    );
    assert_eq!(store.stored(guild_id), vec![StateRevision::new(3)]);
    assert_eq!(
        store
            .get_snapshot_revision(guild_id)
            .await
            .expect("durable watermark"),
        Some(StateRevision::new(3))
    );
}
