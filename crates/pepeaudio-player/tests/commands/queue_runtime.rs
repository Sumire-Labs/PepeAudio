use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use pepeaudio_core::PlayerCommand;
use pepeaudio_player::{PlaybackSource, PlayerError, QueueTrack};

use crate::{
    common::{command, connect, harness, revision, track},
    support::DropMarker,
};

#[tokio::test]
async fn removing_a_queued_track_releases_its_media_lease() {
    let test = harness(Duration::from_mins(5), 4);
    connect(&test.handle).await;
    test.handle
        .enqueue(track("current"), revision(1))
        .await
        .expect("current enqueue");
    let drops = Arc::new(AtomicUsize::new(0));
    let queued = QueueTrack::new(
        "leased queue item",
        None,
        Some(1_000),
        true,
        PlaybackSource::with_lease("memory://leased", DropMarker(drops.clone())),
    );
    let track_id = queued.track_id;
    test.handle
        .enqueue(queued, revision(2))
        .await
        .expect("leased queue item");

    test.handle
        .apply(command(3, PlayerCommand::RemoveQueued { track_id }))
        .await
        .expect("remove leased item");

    assert_eq!(drops.load(Ordering::SeqCst), 1);
    test.runtime.shutdown().await.expect("clean shutdown");
}

#[tokio::test]
async fn queue_and_mailbox_state_use_optimistic_revisions() {
    let test = harness(Duration::from_mins(5), 1);
    connect(&test.handle).await;
    test.handle
        .enqueue(track("one"), revision(1))
        .await
        .expect("first enqueue");

    let stale = test
        .handle
        .enqueue(track("stale"), revision(1))
        .await
        .expect_err("stale revision rejected");
    assert!(matches!(stale, PlayerError::RevisionConflict { .. }));

    test.handle
        .enqueue(track("queued"), revision(2))
        .await
        .expect("queue has one slot");
    let full = test
        .handle
        .enqueue(track("overflow"), revision(3))
        .await
        .expect_err("bounded queue rejects overflow");
    assert!(matches!(full, PlayerError::QueueFull { capacity: 1 }));

    test.runtime.shutdown().await.expect("clean shutdown");
}

#[tokio::test]
async fn single_enqueue_rejects_an_unpublishable_title() {
    let test = harness(Duration::from_mins(5), 1);
    connect(&test.handle).await;
    let mut invalid = track("valid");
    invalid.title = " ".into();

    let error = test
        .handle
        .enqueue(invalid, revision(1))
        .await
        .expect_err("blank public title");

    assert!(matches!(
        error,
        PlayerError::InvalidTrack { field: "title" }
    ));
    assert_eq!(
        test.handle.snapshot().await.expect("snapshot").revision,
        revision(1)
    );
    test.runtime.shutdown().await.expect("clean shutdown");
}
