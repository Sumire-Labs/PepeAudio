use std::convert::Infallible;

use async_trait::async_trait;
use pepeaudio_core::{ChannelId, HrirPresetId, PlayerSnapshot, Volume};

use crate::{PlaybackGeneration, QueueTrack};

/// Voice and decoder operations owned by an integration adapter.
#[async_trait]
pub trait PlaybackPort: Send + 'static {
    type Error: std::error::Error + Send + Sync + 'static;

    async fn connect(&mut self, channel_id: ChannelId) -> Result<(), Self::Error>;
    async fn play(
        &mut self,
        track: &QueueTrack,
        generation: PlaybackGeneration,
    ) -> Result<(), Self::Error>;
    async fn pause(&mut self) -> Result<(), Self::Error>;
    async fn resume(&mut self) -> Result<(), Self::Error>;
    async fn stop(&mut self) -> Result<(), Self::Error>;
    async fn seek(
        &mut self,
        position_ms: u64,
        generation: PlaybackGeneration,
    ) -> Result<(), Self::Error>;
    async fn set_volume(&mut self, volume: Volume) -> Result<(), Self::Error>;
    async fn set_hrir(&mut self, preset: &HrirPresetId) -> Result<(), Self::Error>;
    async fn set_spatial_audio(&mut self, enabled: bool) -> Result<(), Self::Error>;
    async fn disconnect(&mut self) -> Result<(), Self::Error>;
}

/// Ordered publication hook for authoritative snapshots.
#[async_trait]
pub trait SnapshotPublisher: Send + 'static {
    type Error: std::error::Error + Send + Sync + 'static;

    async fn publish(&mut self, snapshot: &PlayerSnapshot) -> Result<(), Self::Error>;
}

#[derive(Debug, Default)]
pub struct NoopPlayback;

#[async_trait]
impl PlaybackPort for NoopPlayback {
    type Error = Infallible;

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
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct NoopSnapshotPublisher;

#[async_trait]
impl SnapshotPublisher for NoopSnapshotPublisher {
    type Error = Infallible;

    async fn publish(&mut self, _: &PlayerSnapshot) -> Result<(), Self::Error> {
        Ok(())
    }
}
