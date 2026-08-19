use pepeaudio_core::{
    CommandEnvelope, PlayerCommand, PlayerSnapshot, PlayerState, RepeatMode, Volume,
};
use uuid::Uuid;

use super::{Actor, shuffle::shuffle_queue, state::unix_now};
use crate::{PlaybackPort, PlayerError, SideEffect, SnapshotPublisher, model::ActiveTrack};

const IDEMPOTENCY_WINDOW: usize = 1_024;

impl<P, S> Actor<P, S>
where
    P: PlaybackPort,
    S: SnapshotPublisher,
{
    pub(super) async fn apply_command(
        &mut self,
        envelope: CommandEnvelope,
    ) -> Result<PlayerSnapshot, PlayerError> {
        if self.recent_idempotency.contains(&envelope.idempotency_key) {
            return Ok(self.current_snapshot());
        }

        envelope.validate_against(&self.current_snapshot(), unix_now())?;
        let idempotency_key = envelope.idempotency_key;
        let result = self.execute(envelope.command, idempotency_key).await?;
        self.remember_idempotency(idempotency_key);
        Ok(result)
    }

    async fn execute(
        &mut self,
        command: PlayerCommand,
        idempotency_key: Uuid,
    ) -> Result<PlayerSnapshot, PlayerError> {
        match command {
            PlayerCommand::Play => self.resume().await,
            PlayerCommand::Pause => self.pause().await,
            PlayerCommand::Stop => self.stop().await,
            PlayerCommand::Skip => self.skip().await,
            PlayerCommand::Previous => self.previous().await,
            PlayerCommand::RemoveQueued { track_id } => self.remove_queued(track_id).await,
            PlayerCommand::MoveQueued {
                track_id,
                before_track_id,
            } => self.move_queued(track_id, before_track_id).await,
            PlayerCommand::Seek { position_ms } => self.seek(position_ms).await,
            PlayerCommand::SetVolume { volume } => self.set_volume(volume).await,
            PlayerCommand::SetRepeat { mode } => self.set_repeat(mode).await,
            PlayerCommand::SetShuffle { enabled } => {
                self.set_shuffle(enabled, idempotency_key).await
            }
            PlayerCommand::SetHrir { preset } => self.set_hrir(preset).await,
            PlayerCommand::SetSpatialAudio { enabled } => self.set_spatial(enabled).await,
            PlayerCommand::Disconnect => self.disconnect().await,
        }
    }

    async fn resume(&mut self) -> Result<PlayerSnapshot, PlayerError> {
        let next_revision = self.next_revision()?;
        self.playback
            .resume()
            .await
            .map_err(|error| PlayerError::side_effect(SideEffect::Resume, error))?;
        let was_idle = self.model.is_idle_connected();
        self.model
            .current
            .as_mut()
            .expect("core validation requires a current track")
            .resume();
        self.model.state = PlayerState::Playing;
        self.finish_change(was_idle, next_revision).await
    }

    async fn pause(&mut self) -> Result<PlayerSnapshot, PlayerError> {
        let next_revision = self.next_revision()?;
        self.playback
            .pause()
            .await
            .map_err(|error| PlayerError::side_effect(SideEffect::Pause, error))?;
        let was_idle = self.model.is_idle_connected();
        self.model
            .current
            .as_mut()
            .expect("core validation requires a current track")
            .pause();
        self.model.state = PlayerState::Paused;
        self.finish_change(was_idle, next_revision).await
    }

    async fn stop(&mut self) -> Result<PlayerSnapshot, PlayerError> {
        let needs_change = self.model.current.is_some()
            || !self.model.queue.is_empty()
            || !self.model.history.is_empty();
        if !needs_change {
            return Ok(self.current_snapshot());
        }
        let next_revision = self.next_revision()?;
        if self.model.current.is_some() {
            self.playback
                .stop()
                .await
                .map_err(|error| PlayerError::side_effect(SideEffect::Stop, error))?;
        }
        let was_idle = self.model.is_idle_connected();
        self.model.current = None;
        self.model.queue.clear();
        self.model.history.clear();
        self.model.state = PlayerState::IdleConnected;
        self.finish_change(was_idle, next_revision).await
    }

    async fn skip(&mut self) -> Result<PlayerSnapshot, PlayerError> {
        let next_revision = self.next_revision()?;
        let replacement = self.model.queue.front().cloned();
        let replacement_generation = if let Some(track) = replacement.as_ref() {
            let generation = self.next_playback_generation()?;
            self.playback
                .play(track, generation)
                .await
                .map_err(|error| PlayerError::side_effect(SideEffect::Play, error))?;
            Some(generation)
        } else {
            self.playback
                .stop()
                .await
                .map_err(|error| PlayerError::side_effect(SideEffect::Stop, error))?;
            None
        };

        let was_idle = self.model.is_idle_connected();
        if let Some(current) = self.model.current.take() {
            self.push_history(current.track);
        }
        self.model.current = self
            .model
            .queue
            .pop_front()
            .zip(replacement_generation)
            .map(|(track, generation)| ActiveTrack::playing(track, generation));
        self.model.state = if self.model.current.is_some() {
            PlayerState::Playing
        } else {
            PlayerState::IdleConnected
        };
        self.finish_change(was_idle, next_revision).await
    }

    async fn previous(&mut self) -> Result<PlayerSnapshot, PlayerError> {
        let previous = self
            .model
            .history
            .back()
            .cloned()
            .expect("core validation requires history");
        let queued_previous = self
            .model
            .queue
            .iter()
            .position(|track| track.track_id == previous.track_id);
        let resulting_queue_len = self
            .model
            .queue
            .len()
            .saturating_sub(usize::from(queued_previous.is_some()))
            .saturating_add(usize::from(self.model.current.is_some()));
        if resulting_queue_len > self.config.max_queued_tracks() {
            return Err(PlayerError::QueueFull {
                capacity: self.config.max_queued_tracks(),
            });
        }
        let next_revision = self.next_revision()?;
        let generation = self.next_playback_generation()?;
        self.playback
            .play(&previous, generation)
            .await
            .map_err(|error| PlayerError::side_effect(SideEffect::Play, error))?;

        let was_idle = self.model.is_idle_connected();
        self.model.history.pop_back();
        if let Some(index) = queued_previous {
            self.model.queue.remove(index);
        }
        if let Some(current) = self.model.current.take() {
            self.model.queue.push_front(current.track);
        }
        self.model.current = Some(ActiveTrack::playing(previous, generation));
        self.model.state = PlayerState::Playing;
        self.finish_change(was_idle, next_revision).await
    }

    async fn seek(&mut self, position_ms: u64) -> Result<PlayerSnapshot, PlayerError> {
        let next_revision = self.next_revision()?;
        let generation = self.next_playback_generation()?;
        self.playback
            .seek(position_ms, generation)
            .await
            .map_err(|error| PlayerError::side_effect(SideEffect::Seek, error))?;
        let was_idle = self.model.is_idle_connected();
        let advancing = self.model.state == PlayerState::Playing;
        self.model
            .current
            .as_mut()
            .expect("core validation requires current track")
            .seek(position_ms, advancing, generation);
        self.finish_change(was_idle, next_revision).await
    }

    async fn set_volume(&mut self, volume: Volume) -> Result<PlayerSnapshot, PlayerError> {
        if self.model.volume == volume {
            return Ok(self.current_snapshot());
        }
        let next_revision = self.next_revision()?;
        if self.model.voice_channel_id.is_some() {
            self.playback
                .set_volume(volume)
                .await
                .map_err(|error| PlayerError::side_effect(SideEffect::SetVolume, error))?;
        }
        let was_idle = self.model.is_idle_connected();
        self.model.volume = volume;
        self.finish_change(was_idle, next_revision).await
    }

    async fn set_repeat(&mut self, mode: RepeatMode) -> Result<PlayerSnapshot, PlayerError> {
        if self.model.repeat_mode == mode {
            return Ok(self.current_snapshot());
        }
        let next_revision = self.next_revision()?;
        let was_idle = self.model.is_idle_connected();
        self.model.repeat_mode = mode;
        self.finish_change(was_idle, next_revision).await
    }

    async fn set_shuffle(
        &mut self,
        enabled: bool,
        idempotency_key: Uuid,
    ) -> Result<PlayerSnapshot, PlayerError> {
        if self.model.shuffle_enabled == enabled {
            return Ok(self.current_snapshot());
        }
        let next_revision = self.next_revision()?;
        let was_idle = self.model.is_idle_connected();
        if enabled {
            shuffle_queue(
                &mut self.model.queue,
                super::shuffle::seed_from_uuid(idempotency_key),
            );
        }
        self.model.shuffle_enabled = enabled;
        self.finish_change(was_idle, next_revision).await
    }

    async fn set_hrir(
        &mut self,
        preset: pepeaudio_core::HrirPresetId,
    ) -> Result<PlayerSnapshot, PlayerError> {
        if self.model.hrir_preset.as_ref() == Some(&preset) {
            return Ok(self.current_snapshot());
        }
        let next_revision = self.next_revision()?;
        if self.model.voice_channel_id.is_some() {
            self.playback
                .set_hrir(&preset)
                .await
                .map_err(|error| PlayerError::side_effect(SideEffect::SetHrir, error))?;
        }
        let was_idle = self.model.is_idle_connected();
        self.model.hrir_preset = Some(preset);
        self.finish_change(was_idle, next_revision).await
    }

    async fn set_spatial(&mut self, enabled: bool) -> Result<PlayerSnapshot, PlayerError> {
        if self.model.spatial_audio_enabled == enabled {
            return Ok(self.current_snapshot());
        }
        let next_revision = self.next_revision()?;
        if self.model.voice_channel_id.is_some() {
            self.playback
                .set_spatial_audio(enabled)
                .await
                .map_err(|error| PlayerError::side_effect(SideEffect::SetSpatialAudio, error))?;
        }
        let was_idle = self.model.is_idle_connected();
        self.model.spatial_audio_enabled = enabled;
        self.finish_change(was_idle, next_revision).await
    }

    async fn disconnect(&mut self) -> Result<PlayerSnapshot, PlayerError> {
        if self.model.voice_channel_id.is_none() {
            return Ok(self.current_snapshot());
        }
        let next_revision = self.next_revision()?;
        self.playback
            .disconnect()
            .await
            .map_err(|error| PlayerError::side_effect(SideEffect::Disconnect, error))?;
        let was_idle = self.model.is_idle_connected();
        self.model.voice_channel_id = None;
        self.model.current = None;
        self.model.queue.clear();
        self.model.history.clear();
        self.model.state = PlayerState::Disconnected;
        self.finish_change(was_idle, next_revision).await
    }

    fn remember_idempotency(&mut self, key: Uuid) {
        if self.recent_idempotency.len() == IDEMPOTENCY_WINDOW {
            self.recent_idempotency.pop_front();
        }
        self.recent_idempotency.push_back(key);
    }
}
