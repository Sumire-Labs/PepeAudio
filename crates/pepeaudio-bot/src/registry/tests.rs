use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use async_trait::async_trait;
use pepeaudio_core::{ChannelId, HrirPresetId, StateRevision, Volume};
use pepeaudio_player::{
    DEFAULT_IDLE_TIMEOUT, NoopPlayback, NoopSnapshotPublisher, PlaybackGeneration, PlaybackPort,
    PlayerConfig, PlayerEvent, QueueTrack, spawn_player,
};
use thiserror::Error;

use super::*;

#[derive(Default)]
struct CountingFactory {
    creations: AtomicUsize,
}

#[async_trait]
impl PlayerFactory for CountingFactory {
    async fn create(&self, guild_id: GuildId) -> Result<PlayerHandle, RegistryError> {
        self.creations.fetch_add(1, Ordering::Relaxed);
        let runtime = spawn_player(
            guild_id,
            PlayerConfig::default(),
            NoopPlayback,
            NoopSnapshotPublisher,
        );
        Ok(runtime.handle())
    }
}

#[tokio::test(start_paused = true)]
async fn idle_stale_handle_is_recreated() {
    let factory = Arc::new(CountingFactory::default());
    let registry = PlayerRegistry::new(factory.clone());
    let guild_id = guild();
    let first = registry.get_or_create(guild_id).await.expect("first actor");
    let mut events = first.subscribe();
    first
        .connect(
            ChannelId::new(24).expect("valid channel"),
            StateRevision::INITIAL,
        )
        .await
        .expect("connect actor");
    tokio::time::advance(DEFAULT_IDLE_TIMEOUT).await;
    wait_for_idle_disconnect(&mut events).await;

    let replacement = registry
        .get_or_create(guild_id)
        .await
        .expect("replacement actor");

    assert_eq!(factory.creations.load(Ordering::Relaxed), 2);
    assert!(replacement.snapshot().await.is_ok());
    registry
        .remove_and_shutdown(guild_id)
        .await
        .expect("cleanup replacement");
}

#[tokio::test]
async fn permanent_guild_removal_clears_and_stops_the_actor() {
    let factory = Arc::new(CountingFactory::default());
    let registry = PlayerRegistry::new(factory.clone());
    let guild_id = guild();
    let removed = registry.get_or_create(guild_id).await.expect("guild actor");

    assert!(
        registry
            .remove_and_shutdown(guild_id)
            .await
            .expect("permanent removal")
    );
    assert!(registry.get(guild_id).await.is_none());
    assert!(matches!(
        removed.snapshot().await,
        Err(PlayerError::ActorStopped)
    ));

    let replacement = registry
        .get_or_create(guild_id)
        .await
        .expect("later guild rejoin creates a new actor");
    assert_eq!(factory.creations.load(Ordering::Relaxed), 2);
    replacement.shutdown().await.expect("cleanup");
}

#[tokio::test]
async fn process_shutdown_drains_live_and_already_stopped_entries() {
    let factory = Arc::new(CountingFactory::default());
    let registry = PlayerRegistry::new(factory);
    let stopped = registry.get_or_create(guild()).await.expect("stale actor");
    stopped.shutdown().await.expect("stop actor before drain");
    let live_guild = GuildId::new(43).expect("valid live guild");
    let live = registry
        .get_or_create(live_guild)
        .await
        .expect("live actor");

    registry.shutdown_all().await.expect("drain registry");

    assert!(registry.get(guild()).await.is_none());
    assert!(registry.get(live_guild).await.is_none());
    assert!(matches!(
        live.snapshot().await,
        Err(PlayerError::ActorStopped)
    ));
}

#[tokio::test(start_paused = true)]
async fn failed_voice_cleanup_remains_registered_for_a_later_retry() {
    let factory = Arc::new(RecoveringFactory::default());
    let registry = PlayerRegistry::new(factory.clone());
    let guild_id = guild();
    let player = registry.get_or_create(guild_id).await.expect("guild actor");
    player
        .connect(
            ChannelId::new(24).expect("valid channel"),
            StateRevision::INITIAL,
        )
        .await
        .expect("connect actor");

    let first = registry.remove_and_shutdown(guild_id).await;

    assert!(matches!(first, Err(RegistryError::Shutdown(_))));
    assert!(registry.get(guild_id).await.is_some());
    assert_eq!(factory.disconnects.load(Ordering::Relaxed), 3);

    assert!(
        registry
            .remove_and_shutdown(guild_id)
            .await
            .expect("cleanup retry")
    );
    assert!(registry.get(guild_id).await.is_none());
    assert_eq!(factory.disconnects.load(Ordering::Relaxed), 4);
}

#[test]
fn voice_disconnect_failure_is_not_reported_as_a_successful_removal() {
    let report = ShutdownReport {
        disconnect_error: Some("voice cleanup failed".into()),
        final_revision: StateRevision::new(4),
    };

    assert!(matches!(
        completed_shutdown(report),
        Err(RegistryError::Shutdown(message)) if message == "voice cleanup failed"
    ));
}

#[derive(Default)]
struct RecoveringFactory {
    disconnects: Arc<AtomicUsize>,
}

#[async_trait]
impl PlayerFactory for RecoveringFactory {
    async fn create(&self, guild_id: GuildId) -> Result<PlayerHandle, RegistryError> {
        let runtime = spawn_player(
            guild_id,
            PlayerConfig::default(),
            FailsOneShutdown {
                disconnects: Arc::clone(&self.disconnects),
            },
            NoopSnapshotPublisher,
        );
        Ok(runtime.handle())
    }
}

struct FailsOneShutdown {
    disconnects: Arc<AtomicUsize>,
}

#[derive(Debug, Error)]
#[error("injected voice cleanup failure")]
struct VoiceCleanupFailure;

#[async_trait]
impl PlaybackPort for FailsOneShutdown {
    type Error = VoiceCleanupFailure;

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
        let attempt = self.disconnects.fetch_add(1, Ordering::Relaxed) + 1;
        if attempt <= 3 {
            Err(VoiceCleanupFailure)
        } else {
            Ok(())
        }
    }
}

fn guild() -> GuildId {
    GuildId::new(42).expect("valid guild")
}

async fn wait_for_idle_disconnect(events: &mut tokio::sync::broadcast::Receiver<PlayerEvent>) {
    loop {
        if matches!(
            events.recv().await.expect("event channel remains open"),
            PlayerEvent::IdleDisconnected { .. }
        ) {
            return;
        }
    }
}
