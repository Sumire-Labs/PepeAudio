use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use pepeaudio_audio::{HorizontalStereoPair, LinearGain, PreparedRenderer, RenderMode};
use pepeaudio_core::{ChannelId, HrirPresetId, Volume};
use pepeaudio_player::{PlaybackGeneration, PlaybackPort, QueueTrack};
use serenity::model::id::{ChannelId as SerenityChannelId, GuildId as SerenityGuildId};

use super::SongbirdPlayback;
use crate::{PipelineError, PipelineResult, dsp::DspMutation};

impl SongbirdPlayback {
    /// Applies a validated horizontal stereo center/width pair to the active
    /// worker as a new clockwise-orbit origin and to all future tracks.
    ///
    /// # Errors
    ///
    /// Returns an error when the current worker rejects the mutation.
    pub async fn set_orbit_position(
        &mut self,
        position: HorizontalStereoPair,
    ) -> PipelineResult<()> {
        if let Some(active) = &self.active
            && active.lifecycle.accepts_dsp_control()
        {
            active.dsp.apply(DspMutation::Orbit(position)).await?;
        }
        self.state.orbit_origin = position;
        Ok(())
    }

    async fn connect_voice(&mut self, channel_id: ChannelId) -> PipelineResult<()> {
        let guild = SerenityGuildId::new(self.guild_id.get());
        let channel = SerenityChannelId::new(channel_id.get());
        if let Ok(call) = self.manager.join(guild, channel).await {
            self.call = Some(call);
            Ok(())
        } else {
            if self.call.is_none() {
                let _ = self.manager.remove(guild).await;
            }
            Err(PipelineError::Voice)
        }
    }

    async fn play_track(
        &mut self,
        track: &QueueTrack,
        generation: PlaybackGeneration,
    ) -> PipelineResult<()> {
        if self.call.is_none() {
            return Err(PipelineError::NotConnected);
        }
        let source = self.dependencies.resolver.resolve(track).await?;
        let replacement_slot = self
            .active
            .as_ref()
            .map(|active| active.process_slot.clone());
        let prepared = self
            .prepare_track(
                track,
                source,
                Duration::ZERO,
                false,
                generation,
                replacement_slot.as_ref(),
            )
            .await?;
        self.replace_active(prepared).await
    }

    fn pause_track(&mut self) -> PipelineResult<()> {
        let active = self.active.as_mut().ok_or(PipelineError::NoActiveTrack)?;
        active
            .handle
            .pause()
            .map_err(|_| PipelineError::TrackControl)?;
        active.paused = true;
        Ok(())
    }

    fn resume_track(&mut self) -> PipelineResult<()> {
        let active = self.active.as_mut().ok_or(PipelineError::NoActiveTrack)?;
        active
            .handle
            .play()
            .map_err(|_| PipelineError::TrackControl)?;
        active.paused = false;
        Ok(())
    }

    async fn seek_track(
        &mut self,
        position_ms: u64,
        generation: PlaybackGeneration,
    ) -> PipelineResult<()> {
        let active = self.active.as_ref().ok_or(PipelineError::NoActiveTrack)?;
        let queue_track = active.queue_track.clone();
        let source = active.source.clone();
        let paused = active.paused;
        let replacement_slot = active.process_slot.clone();
        let prepared = self
            .prepare_track(
                &queue_track,
                source,
                Duration::from_millis(position_ms),
                paused,
                generation,
                Some(&replacement_slot),
            )
            .await?;
        self.replace_active(prepared).await
    }

    async fn update_volume(&mut self, volume: Volume) -> PipelineResult<()> {
        let gain = LinearGain::new(volume.linear_gain())?;
        if let Some(active) = &self.active
            && active.lifecycle.accepts_dsp_control()
        {
            active.dsp.apply(DspMutation::Gain(gain)).await?;
        }
        self.state.gain = gain;
        Ok(())
    }

    async fn update_hrir(&mut self, preset: &HrirPresetId) -> PipelineResult<()> {
        let prepared = self.dependencies.hrirs.get(preset).await?;
        let active_accepts_control = self
            .active
            .as_ref()
            .is_some_and(|active| active.lifecycle.accepts_dsp_control());
        if active_accepts_control {
            let preset = Arc::clone(&prepared);
            let position = self.state.orbit_origin;
            let replacement = tokio::task::spawn_blocking(move || {
                PreparedRenderer::new(preset.as_ref(), RenderMode::HorizontalOrbit(position))
            })
            .await
            .map_err(|_| PipelineError::WorkerTask)??;
            if let Some(active) = &self.active
                && active.lifecycle.accepts_dsp_control()
            {
                active.dsp.apply(DspMutation::Preset(replacement)).await?;
            }
        }
        self.state.preset = prepared;
        Ok(())
    }

    async fn update_spatial(&mut self, enabled: bool) -> PipelineResult<()> {
        if let Some(active) = &self.active
            && active.lifecycle.accepts_dsp_control()
        {
            active.dsp.apply(DspMutation::Spatial(enabled)).await?;
        }
        self.state.spatial_enabled = enabled;
        Ok(())
    }

    async fn disconnect_voice(&mut self) -> PipelineResult<()> {
        if self.call.is_some() {
            self.manager
                .remove(SerenityGuildId::new(self.guild_id.get()))
                .await
                .map_err(|_| PipelineError::Voice)?;
        }
        self.stop_active().await?;
        self.call = None;
        Ok(())
    }
}

#[async_trait]
impl PlaybackPort for SongbirdPlayback {
    type Error = PipelineError;

    async fn connect(&mut self, channel_id: ChannelId) -> Result<(), Self::Error> {
        self.connect_voice(channel_id).await
    }

    async fn play(
        &mut self,
        track: &QueueTrack,
        generation: PlaybackGeneration,
    ) -> Result<(), Self::Error> {
        self.play_track(track, generation).await
    }

    async fn pause(&mut self) -> Result<(), Self::Error> {
        self.pause_track()
    }

    async fn resume(&mut self) -> Result<(), Self::Error> {
        self.resume_track()
    }

    async fn stop(&mut self) -> Result<(), Self::Error> {
        self.stop_active().await
    }

    async fn seek(
        &mut self,
        position_ms: u64,
        generation: PlaybackGeneration,
    ) -> Result<(), Self::Error> {
        self.seek_track(position_ms, generation).await
    }

    async fn set_volume(&mut self, volume: Volume) -> Result<(), Self::Error> {
        self.update_volume(volume).await
    }

    async fn set_hrir(&mut self, preset: &HrirPresetId) -> Result<(), Self::Error> {
        self.update_hrir(preset).await
    }

    async fn set_spatial_audio(&mut self, enabled: bool) -> Result<(), Self::Error> {
        self.update_spatial(enabled).await
    }

    async fn disconnect(&mut self) -> Result<(), Self::Error> {
        self.disconnect_voice().await
    }
}
