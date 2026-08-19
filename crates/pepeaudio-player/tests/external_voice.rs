mod common;

use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use async_trait::async_trait;
use pepeaudio_core::{ChannelId, HrirPresetId, PlayerState, Volume};
use pepeaudio_player::{
    NoopSnapshotPublisher, PlaybackGeneration, PlaybackPort, PlayerConfig, QueueTrack, spawn_player,
};
use thiserror::Error;

use common::{PlaybackCall, connect, harness, revision, track};

#[tokio::test]
async fn external_move_updates_channel_without_losing_playback_state() {
    let test = harness(Duration::from_mins(5), 4);
    connect(&test.handle).await;
    let current = track("current");
    let current_id = current.track_id;
    test.handle
        .enqueue(current, revision(1))
        .await
        .expect("current");
    test.handle
        .enqueue(track("queued"), revision(2))
        .await
        .expect("queued");
    let moved = ChannelId::new(999).expect("moved channel");

    let snapshot = test
        .handle
        .reconcile_voice_channel(Some(moved))
        .await
        .expect("external move");

    assert_eq!(snapshot.voice_channel_id, Some(moved));
    assert_eq!(snapshot.state, PlayerState::Playing);
    assert_eq!(
        snapshot.current_track.as_ref().map(|item| item.track_id),
        Some(current_id)
    );
    assert_eq!(snapshot.queued_tracks, 1);
    assert!(
        test.playback
            .calls()
            .await
            .contains(&PlaybackCall::Connect(moved))
    );

    let duplicate = test
        .handle
        .reconcile_voice_channel(Some(moved))
        .await
        .expect("duplicate observation");
    assert_eq!(duplicate.revision, snapshot.revision);
    test.runtime.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn external_disconnect_clears_actor_and_songbird_state() {
    let test = harness(Duration::from_mins(5), 4);
    connect(&test.handle).await;
    test.handle
        .enqueue(track("current"), revision(1))
        .await
        .expect("current");
    test.handle
        .enqueue(track("queued"), revision(2))
        .await
        .expect("queued");

    let snapshot = test
        .handle
        .reconcile_voice_channel(None)
        .await
        .expect("external disconnect");

    assert_eq!(snapshot.state, PlayerState::Disconnected);
    assert!(snapshot.voice_channel_id.is_none());
    assert!(snapshot.current_track.is_none());
    assert_eq!(snapshot.queued_tracks, 0);
    assert!(
        test.playback
            .calls()
            .await
            .contains(&PlaybackCall::Disconnect)
    );
    test.runtime.shutdown().await.expect("shutdown");
}

#[derive(Clone, Default)]
struct RejectMovePlayback(Arc<AtomicUsize>);

#[derive(Debug, Error)]
#[error("voice move rejected")]
struct MoveRejected;

#[async_trait]
impl PlaybackPort for RejectMovePlayback {
    type Error = MoveRejected;

    async fn connect(&mut self, _: ChannelId) -> Result<(), Self::Error> {
        if self.0.fetch_add(1, Ordering::SeqCst) == 0 {
            Ok(())
        } else {
            Err(MoveRejected)
        }
    }

    async fn play(&mut self, _: &QueueTrack, _: PlaybackGeneration) -> Result<(), Self::Error> {
        Ok(())
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

#[tokio::test]
async fn failed_external_move_fails_closed_to_disconnected() {
    let playback = RejectMovePlayback::default();
    let config = PlayerConfig::new(16, 32, 4, Duration::from_mins(5)).expect("config");
    let runtime = spawn_player(common::guild(), config, playback, NoopSnapshotPublisher);
    let handle = runtime.handle();
    connect(&handle).await;
    handle
        .enqueue(track("current"), revision(1))
        .await
        .expect("current");

    let snapshot = handle
        .reconcile_voice_channel(Some(ChannelId::new(999).expect("moved channel")))
        .await
        .expect("failed move still commits authoritative fallback");

    assert_eq!(snapshot.state, PlayerState::Disconnected);
    assert!(snapshot.voice_channel_id.is_none());
    assert!(snapshot.current_track.is_none());
    runtime.shutdown().await.expect("shutdown");
}
