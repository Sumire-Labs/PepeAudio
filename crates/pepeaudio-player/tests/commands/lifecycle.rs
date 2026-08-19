use std::time::Duration;

use pepeaudio_core::PlayerState;

use crate::{
    common::{PlaybackCall, connect, harness, revision, track},
    support::wait_for_shutdown_snapshot,
};

#[tokio::test]
async fn every_committed_revision_is_published_in_order() {
    let test = harness(Duration::from_mins(5), 4);
    connect(&test.handle).await;
    test.handle
        .enqueue(track("one"), revision(1))
        .await
        .expect("enqueue");

    let snapshots = test.publisher.snapshots().await;
    let revisions: Vec<_> = snapshots
        .iter()
        .map(|snapshot| snapshot.revision.get())
        .collect();
    assert_eq!(revisions, vec![0, 1, 2]);

    test.runtime.shutdown().await.expect("clean shutdown");
}

#[tokio::test]
async fn dropping_runtime_still_releases_voice_resources() {
    let test = harness(Duration::from_mins(5), 4);
    connect(&test.handle).await;
    let playback = test.playback.clone();
    drop(test.handle);
    drop(test.runtime);

    playback.wait_for_disconnect().await;
    assert!(playback.calls().await.contains(&PlaybackCall::Disconnect));
}

#[tokio::test]
async fn explicit_shutdown_clears_active_queue_and_disconnects() {
    let test = harness(Duration::from_mins(5), 4);
    let mut events = test.handle.subscribe();
    connect(&test.handle).await;
    test.handle
        .enqueue(track("active"), revision(1))
        .await
        .expect("active track");
    test.handle
        .enqueue(track("queued"), revision(2))
        .await
        .expect("queued track");

    test.handle.shutdown().await.expect("explicit shutdown");

    let terminal = wait_for_shutdown_snapshot(&mut events).await;
    assert_eq!(terminal.state, PlayerState::Disconnected);
    assert!(terminal.voice_channel_id.is_none());
    assert!(terminal.current_track.is_none());
    assert_eq!(terminal.queued_tracks, 0);
    assert!(
        test.playback
            .calls()
            .await
            .contains(&PlaybackCall::Disconnect)
    );
    test.runtime.shutdown().await.expect("join stopped actor");
}
