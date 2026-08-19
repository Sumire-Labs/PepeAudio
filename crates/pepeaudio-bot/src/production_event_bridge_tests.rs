use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use async_trait::async_trait;
use pepeaudio_core::{ChannelId, GuildId, HrirPresetId, StateRevision, Volume};
use pepeaudio_pipeline::{PlaybackEndReason, PlaybackEvent};
use pepeaudio_player::{
    NoopSnapshotPublisher, PlaybackGeneration, PlaybackIdentity, PlaybackPort, PlayerConfig,
    PlayerError, QueueTrack, spawn_player,
};
use thiserror::Error;
use tokio::sync::broadcast;
use uuid::Uuid;

#[derive(Clone, Default)]
struct RecoveringDisconnect {
    attempts: Arc<AtomicUsize>,
}

#[derive(Debug, Error)]
#[error("injected disconnect failure")]
struct DisconnectFailure;

#[async_trait]
impl PlaybackPort for RecoveringDisconnect {
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
        let attempt = self.attempts.fetch_add(1, Ordering::Relaxed) + 1;
        if attempt <= 3 {
            Err(DisconnectFailure)
        } else {
            Ok(())
        }
    }
}

#[tokio::test(start_paused = true)]
async fn lagged_pipeline_keeps_recovering_until_voice_cleanup_succeeds() {
    let guild_id = GuildId::new(77).expect("guild");
    let playback = RecoveringDisconnect::default();
    let attempts = Arc::clone(&playback.attempts);
    let runtime = spawn_player(
        guild_id,
        PlayerConfig::default(),
        playback,
        NoopSnapshotPublisher,
    );
    let handle = runtime.handle();
    handle
        .connect(ChannelId::new(88).expect("channel"), StateRevision::INITIAL)
        .await
        .expect("connect");

    let (sender, receiver) = broadcast::channel(1);
    sender.send(ended_event(guild_id)).expect("first event");
    sender.send(ended_event(guild_id)).expect("second event");
    let bridge = tokio::spawn(super::production_event_bridge::run(
        guild_id,
        receiver,
        handle.clone(),
    ));

    tokio::task::yield_now().await;
    for _ in 0..3 {
        tokio::time::advance(std::time::Duration::from_millis(100)).await;
        tokio::task::yield_now().await;
    }

    assert_eq!(attempts.load(Ordering::Relaxed), 3);
    assert!(
        !bridge.is_finished(),
        "bridge must retain cleanup ownership"
    );
    assert!(handle.snapshot().await.is_ok(), "actor remains retryable");

    tokio::time::advance(std::time::Duration::from_secs(1)).await;
    tokio::task::yield_now().await;
    bridge.await.expect("bridge task");

    assert_eq!(attempts.load(Ordering::Relaxed), 4);
    assert!(matches!(
        handle.snapshot().await,
        Err(PlayerError::ActorStopped)
    ));
    let report = runtime.shutdown().await.expect("join actor");
    assert!(report.disconnect_error.is_none());
}

fn ended_event(guild_id: GuildId) -> PlaybackEvent {
    PlaybackEvent::TrackEnded {
        guild_id,
        identity: PlaybackIdentity::new(Uuid::new_v4(), PlaybackGeneration::new(1)),
        reason: PlaybackEndReason::Natural,
    }
}
