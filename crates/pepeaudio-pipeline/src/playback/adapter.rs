use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use pepeaudio_audio::{HorizontalStereoPair, LinearGain, PreparedHrir};
use pepeaudio_core::{GuildId, Volume};
use pepeaudio_player::PlaybackGeneration;
use songbird::{Call, Songbird};
use tokio::sync::{Mutex, broadcast};

use crate::{
    DecoderFactory, HrirProvider, PipelineConfig, PipelineError, PipelineResult, PlaybackEvent,
    TrackResolver, dsp::DspState, track::ActiveTrack,
};

#[derive(Clone)]
pub struct PipelineDependencies {
    pub(crate) resolver: Arc<dyn TrackResolver>,
    pub(crate) decoder: Arc<dyn DecoderFactory>,
    pub(crate) hrirs: Arc<dyn HrirProvider>,
    pub(crate) initial_hrir: Arc<PreparedHrir>,
}

impl PipelineDependencies {
    /// Creates dependencies whose expensive media/HRIR preparation happens
    /// before PCM reaches the realtime worker.
    #[must_use]
    pub fn new(
        resolver: Arc<dyn TrackResolver>,
        decoder: Arc<dyn DecoderFactory>,
        hrirs: Arc<dyn HrirProvider>,
        initial_hrir: Arc<PreparedHrir>,
    ) -> Self {
        Self {
            resolver,
            decoder,
            hrirs,
            initial_hrir,
        }
    }
}

/// Live position observed directly from Songbird's track handle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlaybackStatus {
    pub track_id: uuid::Uuid,
    /// Seek base plus Songbird's position in the current PCM generation.
    pub position_ms: u64,
    pub paused: bool,
    /// Monotonic adapter generation used for stale-event rejection.
    pub generation: PlaybackGeneration,
}

pub struct SongbirdPlayback {
    pub(crate) manager: Arc<Songbird>,
    pub(crate) guild_id: GuildId,
    pub(crate) dependencies: PipelineDependencies,
    pub(crate) config: PipelineConfig,
    pub(crate) call: Option<Arc<Mutex<Call>>>,
    pub(crate) active: Option<ActiveTrack>,
    pub(crate) state: DspState,
    pub(crate) current_generation: Arc<AtomicU64>,
    pub(crate) events: broadcast::Sender<PlaybackEvent>,
}

impl SongbirdPlayback {
    /// Creates one guild-owned adapter. The manager must be the same instance
    /// registered with Serenity's voice gateway integration.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid buffer policy or an invalid default gain.
    pub fn new(
        manager: Arc<Songbird>,
        guild_id: GuildId,
        dependencies: PipelineDependencies,
        config: PipelineConfig,
    ) -> PipelineResult<Self> {
        let config = config.validate()?;
        let gain = LinearGain::new(Volume::DEFAULT.linear_gain())?;
        let (events, _) = broadcast::channel(config.event_capacity);
        Ok(Self {
            manager,
            guild_id,
            state: DspState {
                preset: Arc::clone(&dependencies.initial_hrir),
                gain,
                spatial_enabled: false,
                orbit_origin: HorizontalStereoPair::FRONT,
            },
            dependencies,
            config,
            call: None,
            active: None,
            current_generation: Arc::new(AtomicU64::new(0)),
            events,
        })
    }

    #[must_use]
    pub fn subscribe_events(&self) -> broadcast::Receiver<PlaybackEvent> {
        self.events.subscribe()
    }

    #[must_use]
    pub const fn guild_id(&self) -> GuildId {
        self.guild_id
    }

    /// # Errors
    ///
    /// Returns an error when no track exists or Songbird discarded it.
    pub async fn status(&self) -> PipelineResult<PlaybackStatus> {
        let active = self.active.as_ref().ok_or(PipelineError::NoActiveTrack)?;
        let info = active
            .handle
            .get_info()
            .await
            .map_err(|_| PipelineError::TrackControl)?;
        let position = active.base_position.saturating_add(info.position);
        Ok(PlaybackStatus {
            track_id: active.queue_track.track_id,
            position_ms: u64::try_from(position.as_millis()).unwrap_or(u64::MAX),
            paused: active.paused,
            generation: active.generation,
        })
    }

    pub(crate) fn publish_generation(&self, generation: PlaybackGeneration) {
        self.current_generation
            .store(generation.get(), Ordering::Release);
    }

    pub(crate) fn invalidate_generation(&self) {
        self.current_generation.store(0, Ordering::Release);
    }
}

impl Drop for SongbirdPlayback {
    fn drop(&mut self) {
        if let Some(active) = self.active.take() {
            drop(active);
        }
    }
}
