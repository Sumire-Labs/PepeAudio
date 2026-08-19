use std::time::Duration;

use async_trait::async_trait;
use pepeaudio_core::{ChannelId, HrirPresetId, Volume};
use pepeaudio_player::{
    PlaybackGeneration, PlaybackPort, PlayerConfig, PlayerError, QueueTrack, spawn_player,
};

use crate::common::{PlaybackCall, PublisherSpy, channel, guild, harness, revision, track};

#[tokio::test]
async fn idle_batch_starts_first_and_preserves_remaining_order() {
    let test = harness(Duration::from_mins(5), 100);
    test.handle
        .connect(channel(), revision(0))
        .await
        .expect("connect");
    let tracks = vec![track("one"), track("two"), track("three")];
    let first_id = tracks[0].track_id;

    let snapshot = test
        .handle
        .enqueue_batch(tracks.clone(), revision(1))
        .await
        .expect("batch");

    assert_eq!(snapshot.revision.get(), 2);
    assert_eq!(
        snapshot
            .current_track
            .as_ref()
            .map(|item| item.title.as_str()),
        Some("one")
    );
    assert_eq!(
        snapshot
            .upcoming_tracks
            .iter()
            .map(|item| item.title.as_str())
            .collect::<Vec<_>>(),
        ["two", "three"]
    );
    assert!(
        test.playback
            .calls()
            .await
            .contains(&PlaybackCall::Play(first_id))
    );
    test.runtime.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn active_batch_is_one_revision_and_does_not_restart_playback() {
    let test = harness(Duration::from_mins(5), 100);
    test.handle
        .connect(channel(), revision(0))
        .await
        .expect("connect");
    let current = track("current");
    let current_id = current.track_id;
    test.handle
        .enqueue(current, revision(1))
        .await
        .expect("current");

    let snapshot = test
        .handle
        .enqueue_batch(vec![track("two"), track("three")], revision(2))
        .await
        .expect("batch");

    assert_eq!(snapshot.revision.get(), 3);
    assert_eq!(snapshot.queued_tracks, 2);
    let plays = test
        .playback
        .calls()
        .await
        .into_iter()
        .filter(|call| matches!(call, PlaybackCall::Play(_)))
        .collect::<Vec<_>>();
    assert_eq!(plays, [PlaybackCall::Play(current_id)]);
    test.runtime.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn duplicate_or_over_capacity_batch_is_atomic() {
    let runtime = spawn_player(
        guild(),
        PlayerConfig::new(16, 16, 1, Duration::from_mins(5)).expect("config"),
        crate::common::PlaybackSpy::default(),
        PublisherSpy::default(),
    );
    let handle = runtime.handle();
    handle
        .connect(channel(), revision(0))
        .await
        .expect("connect");
    let duplicate = track("duplicate");
    let error = handle
        .enqueue_batch(vec![duplicate.clone(), duplicate], revision(1))
        .await
        .expect_err("duplicate");
    assert!(matches!(error, PlayerError::DuplicateTrack { .. }));
    let unchanged = handle.snapshot().await.expect("snapshot");
    assert_eq!(unchanged.revision.get(), 1);
    assert!(unchanged.current_track.is_none());

    let error = handle
        .enqueue_batch(
            vec![track("one"), track("two"), track("three")],
            revision(1),
        )
        .await
        .expect_err("capacity");
    assert!(matches!(error, PlayerError::QueueFull { capacity: 1 }));
    let unchanged = handle.snapshot().await.expect("snapshot");
    assert_eq!(unchanged.revision.get(), 1);
    assert!(unchanged.current_track.is_none());
    runtime.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn playback_failure_leaves_the_batch_uncommitted() {
    let runtime = spawn_player(
        guild(),
        PlayerConfig::default(),
        RejectPlay,
        PublisherSpy::default(),
    );
    let handle = runtime.handle();
    handle
        .connect(channel(), revision(0))
        .await
        .expect("connect");

    handle
        .enqueue_batch(vec![track("one"), track("two")], revision(1))
        .await
        .expect_err("playback fails");

    let unchanged = handle.snapshot().await.expect("snapshot");
    assert_eq!(unchanged.revision.get(), 1);
    assert!(unchanged.current_track.is_none());
    assert!(unchanged.upcoming_tracks.is_empty());
    runtime.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn oversized_batch_is_rejected_by_capacity_before_duplicate_work() {
    let runtime = spawn_player(
        guild(),
        PlayerConfig::new(16, 16, 1, Duration::from_mins(5)).expect("config"),
        crate::common::PlaybackSpy::default(),
        PublisherSpy::default(),
    );
    let handle = runtime.handle();
    handle
        .connect(channel(), revision(0))
        .await
        .expect("connect");
    let repeated = track("repeated");
    let oversized = vec![repeated; 10_000];

    let error = handle
        .enqueue_batch(oversized, revision(1))
        .await
        .expect_err("capacity is checked before duplicates");

    assert!(matches!(error, PlayerError::QueueFull { capacity: 1 }));
    let unchanged = handle.snapshot().await.expect("snapshot");
    assert_eq!(unchanged.revision.get(), 1);
    assert!(unchanged.current_track.is_none());
    runtime.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn unpublishable_title_is_rejected_before_playback() {
    let test = harness(Duration::from_mins(5), 4);
    test.handle
        .connect(channel(), revision(0))
        .await
        .expect("connect");
    let mut invalid = track("valid");
    invalid.title = "\u{0000}".into();

    let error = test
        .handle
        .enqueue_batch(vec![invalid], revision(1))
        .await
        .expect_err("invalid public title");

    assert!(matches!(
        error,
        PlayerError::InvalidTrack { field: "title" }
    ));
    assert!(
        test.playback
            .calls()
            .await
            .iter()
            .all(|call| !matches!(call, PlaybackCall::Play(_)))
    );
    test.runtime.shutdown().await.expect("shutdown");
}

#[derive(Debug, thiserror::Error)]
#[error("play rejected")]
struct PlayRejected;

struct RejectPlay;

#[async_trait]
impl PlaybackPort for RejectPlay {
    type Error = PlayRejected;

    async fn connect(&mut self, _: ChannelId) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn play(&mut self, _: &QueueTrack, _: PlaybackGeneration) -> Result<(), Self::Error> {
        Err(PlayRejected)
    }

    async fn pause(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn resume(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn seek(&mut self, _: u64, _: PlaybackGeneration) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn set_volume(&mut self, _: Volume) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn set_hrir(&mut self, _: &HrirPresetId) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn set_spatial_audio(&mut self, _: bool) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
