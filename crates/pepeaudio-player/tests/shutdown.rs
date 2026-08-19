use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use pepeaudio_core::{ChannelId, HrirPresetId, Volume};
use pepeaudio_player::{
    NoopSnapshotPublisher, PlaybackGeneration, PlaybackPort, PlayerConfig, QueueTrack, spawn_player,
};
use thiserror::Error;
use tokio::sync::Mutex;

#[derive(Clone)]
struct RetryingDisconnect {
    attempts: Arc<Mutex<usize>>,
    failures_before_success: usize,
}

impl RetryingDisconnect {
    fn new(failures_before_success: usize) -> Self {
        Self {
            attempts: Arc::new(Mutex::new(0)),
            failures_before_success,
        }
    }
}

#[derive(Debug, Error)]
#[error("injected disconnect failure")]
struct DisconnectFailure;

#[async_trait]
impl PlaybackPort for RetryingDisconnect {
    type Error = DisconnectFailure;

    async fn connect(&mut self, _: ChannelId) -> Result<(), Self::Error> {
        Ok(())
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
        let mut attempts = self.attempts.lock().await;
        *attempts += 1;
        if *attempts <= self.failures_before_success {
            Err(DisconnectFailure)
        } else {
            Ok(())
        }
    }
}

#[tokio::test(start_paused = true)]
async fn graceful_shutdown_retries_transient_voice_cleanup() {
    let playback = RetryingDisconnect::new(2);
    let attempts = Arc::clone(&playback.attempts);
    let runtime = spawn_player(
        pepeaudio_core::GuildId::new(1).expect("guild"),
        PlayerConfig::new(8, 8, 4, Duration::from_mins(5)).expect("config"),
        playback,
        NoopSnapshotPublisher,
    );
    runtime
        .handle()
        .connect(
            ChannelId::new(2).expect("channel"),
            pepeaudio_core::StateRevision::INITIAL,
        )
        .await
        .expect("connected");

    let report = runtime.shutdown().await.expect("actor joins");

    assert!(report.disconnect_error.is_none());
    assert_eq!(*attempts.lock().await, 3);
}

#[tokio::test(start_paused = true)]
async fn failed_shutdown_keeps_the_actor_available_for_cleanup_retry() {
    let playback = RetryingDisconnect::new(3);
    let attempts = Arc::clone(&playback.attempts);
    let runtime = spawn_player(
        pepeaudio_core::GuildId::new(1).expect("guild"),
        PlayerConfig::new(8, 8, 4, Duration::from_mins(5)).expect("config"),
        playback,
        NoopSnapshotPublisher,
    );
    let handle = runtime.handle();
    handle
        .connect(
            ChannelId::new(2).expect("channel"),
            pepeaudio_core::StateRevision::INITIAL,
        )
        .await
        .expect("connected");

    let failed = handle.shutdown().await.expect("shutdown response");

    assert!(failed.disconnect_error.is_some());
    assert!(
        handle
            .snapshot()
            .await
            .expect("actor remains live")
            .voice_channel_id
            .is_some()
    );

    let recovered = handle.shutdown().await.expect("retry response");
    assert!(recovered.disconnect_error.is_none());
    assert_eq!(*attempts.lock().await, 4);
    runtime.shutdown().await.expect("actor joins");
}
