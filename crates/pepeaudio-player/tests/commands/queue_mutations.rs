use std::time::Duration;

use pepeaudio_core::{PlayerCommand, PlayerState};
use pepeaudio_player::PlayerError;
use uuid::Uuid;

use crate::{
    common::{PlaybackCall, command, connect, harness, revision, track},
    support::upcoming_ids,
};

#[tokio::test]
async fn stop_clears_current_track_and_entire_queue() {
    let test = harness(Duration::from_mins(5), 4);
    connect(&test.handle).await;
    test.handle
        .enqueue(track("one"), revision(1))
        .await
        .expect("first enqueue");
    test.handle
        .enqueue(track("two"), revision(2))
        .await
        .expect("second enqueue");

    let stopped = test
        .handle
        .apply(command(3, PlayerCommand::Stop))
        .await
        .expect("stop");
    assert_eq!(stopped.state, PlayerState::IdleConnected);
    assert!(stopped.current_track.is_none());
    assert_eq!(stopped.queued_tracks, 0);
    assert!(test.playback.calls().await.contains(&PlaybackCall::Stop));

    test.runtime.shutdown().await.expect("clean shutdown");
}

#[tokio::test]
async fn remove_queued_preserves_current_track_and_remaining_order() {
    let test = harness(Duration::from_mins(5), 4);
    connect(&test.handle).await;
    let current = track("current");
    let first = track("first");
    let removed = track("remove me");
    let last = track("last");
    test.handle
        .enqueue(current.clone(), revision(1))
        .await
        .expect("current enqueue");
    test.handle
        .enqueue(first.clone(), revision(2))
        .await
        .expect("first queued");
    test.handle
        .enqueue(removed.clone(), revision(3))
        .await
        .expect("removable queued");
    test.handle
        .enqueue(last.clone(), revision(4))
        .await
        .expect("last queued");

    let updated = test
        .handle
        .apply(command(
            5,
            PlayerCommand::RemoveQueued {
                track_id: removed.track_id,
            },
        ))
        .await
        .expect("remove queued");

    assert_eq!(updated.revision, revision(6));
    assert_eq!(
        updated.current_track.as_ref().map(|track| track.track_id),
        Some(current.track_id)
    );
    assert_eq!(
        updated
            .upcoming_tracks
            .iter()
            .map(|track| track.track_id)
            .collect::<Vec<_>>(),
        vec![first.track_id, last.track_id]
    );
    assert_eq!(updated.queued_tracks, 2);
    assert_eq!(
        test.publisher
            .snapshots()
            .await
            .last()
            .expect("published removal")
            .revision,
        revision(6)
    );

    test.runtime.shutdown().await.expect("clean shutdown");
}

#[tokio::test]
async fn remove_queued_rejects_current_or_missing_tracks_without_a_revision_change() {
    let test = harness(Duration::from_mins(5), 4);
    connect(&test.handle).await;
    let current = track("current");
    test.handle
        .enqueue(current.clone(), revision(1))
        .await
        .expect("current enqueue");

    for track_id in [current.track_id, Uuid::new_v4()] {
        let error = test
            .handle
            .apply(command(2, PlayerCommand::RemoveQueued { track_id }))
            .await
            .expect_err("only upcoming tracks may be removed");
        assert!(matches!(
            error,
            PlayerError::InvalidCommand(
                pepeaudio_core::CommandValidationError::QueuedTrackNotFound { .. }
            )
        ));
    }
    assert_eq!(
        test.handle.snapshot().await.expect("snapshot").revision,
        revision(2)
    );

    test.runtime.shutdown().await.expect("clean shutdown");
}

#[tokio::test]
async fn move_queued_reorders_by_stable_identity_and_avoids_noop_revisions() {
    let test = harness(Duration::from_mins(5), 4);
    connect(&test.handle).await;
    let current = track("current");
    let first = track("first");
    let second = track("second");
    let third = track("third");
    for (track, expected_revision) in [
        (current, 1),
        (first.clone(), 2),
        (second.clone(), 3),
        (third.clone(), 4),
    ] {
        test.handle
            .enqueue(track, revision(expected_revision))
            .await
            .expect("enqueue ordered fixture");
    }

    let promoted = test
        .handle
        .apply(command(
            5,
            PlayerCommand::MoveQueued {
                track_id: third.track_id,
                before_track_id: Some(first.track_id),
            },
        ))
        .await
        .expect("move last track to the front");
    assert_eq!(promoted.revision, revision(6));
    assert_eq!(
        upcoming_ids(&promoted),
        vec![third.track_id, first.track_id, second.track_id]
    );

    let demoted = test
        .handle
        .apply(command(
            6,
            PlayerCommand::MoveQueued {
                track_id: third.track_id,
                before_track_id: None,
            },
        ))
        .await
        .expect("move first queued track to the end");
    assert_eq!(demoted.revision, revision(7));
    assert_eq!(
        upcoming_ids(&demoted),
        vec![first.track_id, second.track_id, third.track_id]
    );

    for no_change in [
        PlayerCommand::MoveQueued {
            track_id: first.track_id,
            before_track_id: Some(second.track_id),
        },
        PlayerCommand::MoveQueued {
            track_id: second.track_id,
            before_track_id: Some(second.track_id),
        },
    ] {
        let unchanged = test
            .handle
            .apply(command(7, no_change))
            .await
            .expect("an already satisfied move is idempotent");
        assert_eq!(unchanged.revision, revision(7));
    }

    test.runtime.shutdown().await.expect("clean shutdown");
}
