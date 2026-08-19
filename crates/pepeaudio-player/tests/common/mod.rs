#![allow(dead_code)]

use std::{convert::Infallible, sync::Arc, time::Duration};

use async_trait::async_trait;
use pepeaudio_core::{
    ChannelId, CommandEnvelope, GuildId, HrirPresetId, PlayerCommand, PlayerSnapshot,
    StateRevision, UnixTimeMillis, UserId, Volume,
};
use pepeaudio_player::{
    PlaybackGeneration, PlaybackPort, PlaybackSource, PlayerConfig, PlayerHandle, PlayerRuntime,
    QueueTrack, SnapshotPublisher, spawn_player,
};
use tokio::sync::{Mutex, Notify};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PlaybackCall {
    Connect(ChannelId),
    Play(uuid::Uuid),
    Pause,
    Resume,
    Stop,
    Seek(u64),
    Volume(Volume),
    Hrir(String),
    Spatial(bool),
    Disconnect,
}

#[derive(Clone, Default)]
pub(crate) struct PlaybackSpy {
    calls: Arc<Mutex<Vec<PlaybackCall>>>,
    disconnected: Arc<Notify>,
}

impl PlaybackSpy {
    pub(crate) async fn calls(&self) -> Vec<PlaybackCall> {
        self.calls.lock().await.clone()
    }

    pub(crate) async fn wait_for_disconnect(&self) {
        self.disconnected.notified().await;
    }

    async fn record(&self, call: PlaybackCall) {
        self.calls.lock().await.push(call);
    }
}

#[async_trait]
impl PlaybackPort for PlaybackSpy {
    type Error = Infallible;

    async fn connect(&mut self, channel_id: ChannelId) -> Result<(), Self::Error> {
        self.record(PlaybackCall::Connect(channel_id)).await;
        Ok(())
    }

    async fn play(&mut self, track: &QueueTrack, _: PlaybackGeneration) -> Result<(), Self::Error> {
        self.record(PlaybackCall::Play(track.track_id)).await;
        Ok(())
    }

    async fn pause(&mut self) -> Result<(), Self::Error> {
        self.record(PlaybackCall::Pause).await;
        Ok(())
    }

    async fn resume(&mut self) -> Result<(), Self::Error> {
        self.record(PlaybackCall::Resume).await;
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), Self::Error> {
        self.record(PlaybackCall::Stop).await;
        Ok(())
    }

    async fn seek(&mut self, position_ms: u64, _: PlaybackGeneration) -> Result<(), Self::Error> {
        self.record(PlaybackCall::Seek(position_ms)).await;
        Ok(())
    }

    async fn set_volume(&mut self, volume: Volume) -> Result<(), Self::Error> {
        self.record(PlaybackCall::Volume(volume)).await;
        Ok(())
    }

    async fn set_hrir(&mut self, preset: &HrirPresetId) -> Result<(), Self::Error> {
        self.record(PlaybackCall::Hrir(preset.to_string())).await;
        Ok(())
    }

    async fn set_spatial_audio(&mut self, enabled: bool) -> Result<(), Self::Error> {
        self.record(PlaybackCall::Spatial(enabled)).await;
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), Self::Error> {
        self.record(PlaybackCall::Disconnect).await;
        self.disconnected.notify_one();
        Ok(())
    }
}

#[derive(Clone, Default)]
pub(crate) struct PublisherSpy(Arc<Mutex<Vec<PlayerSnapshot>>>);

impl PublisherSpy {
    pub(crate) async fn snapshots(&self) -> Vec<PlayerSnapshot> {
        self.0.lock().await.clone()
    }
}

#[async_trait]
impl SnapshotPublisher for PublisherSpy {
    type Error = Infallible;

    async fn publish(&mut self, snapshot: &PlayerSnapshot) -> Result<(), Self::Error> {
        self.0.lock().await.push(snapshot.clone());
        Ok(())
    }
}

pub(crate) struct Harness {
    pub(crate) runtime: PlayerRuntime,
    pub(crate) handle: PlayerHandle,
    pub(crate) playback: PlaybackSpy,
    pub(crate) publisher: PublisherSpy,
}

pub(crate) fn harness(idle_timeout: Duration, queue_capacity: usize) -> Harness {
    let playback = PlaybackSpy::default();
    let publisher = PublisherSpy::default();
    let config = PlayerConfig::new(16, 32, queue_capacity, idle_timeout).expect("valid config");
    let runtime = spawn_player(guild(), config, playback.clone(), publisher.clone());
    let handle = runtime.handle();
    Harness {
        runtime,
        handle,
        playback,
        publisher,
    }
}

pub(crate) const fn revision(value: u64) -> StateRevision {
    StateRevision::new(value)
}

pub(crate) fn guild() -> GuildId {
    GuildId::new(123).expect("valid guild")
}

pub(crate) fn channel() -> ChannelId {
    ChannelId::new(456).expect("valid channel")
}

pub(crate) fn track(title: &str) -> QueueTrack {
    QueueTrack::new(
        title,
        Some(UserId::new(789).expect("valid user")),
        Some(180_000),
        true,
        PlaybackSource::new(format!("memory://{title}")),
    )
}

pub(crate) fn command(expected: u64, command: PlayerCommand) -> CommandEnvelope {
    CommandEnvelope::new(
        guild(),
        Some(UserId::new(789).expect("valid user")),
        revision(expected),
        UnixTimeMillis::new(u64::MAX),
        command,
    )
}

pub(crate) async fn connect(handle: &PlayerHandle) -> PlayerSnapshot {
    handle
        .connect(channel(), StateRevision::INITIAL)
        .await
        .expect("connect succeeds")
}
