use std::collections::HashSet;

use pepeaudio_core::{
    ChannelId, CommandValidationError, PlayerSnapshot, PlayerState, StateRevision,
};
use uuid::Uuid;

use super::{
    Actor,
    shuffle::{insert_shuffled, seed_from_uuid},
};
use crate::{
    PlaybackPort, PlayerError, QueueTrack, SideEffect, SnapshotPublisher, model::ActiveTrack,
};

impl<P, S> Actor<P, S>
where
    P: PlaybackPort,
    S: SnapshotPublisher,
{
    pub(super) async fn connect(
        &mut self,
        channel_id: ChannelId,
        expected_revision: StateRevision,
    ) -> Result<PlayerSnapshot, PlayerError> {
        self.check_revision(expected_revision)?;
        if let Some(connected) = self.model.voice_channel_id {
            return if connected == channel_id {
                Ok(self.current_snapshot())
            } else {
                Err(PlayerError::VoiceChannelMismatch {
                    connected,
                    requested: channel_id,
                })
            };
        }

        let next_revision = self.next_revision()?;
        self.playback
            .connect(channel_id)
            .await
            .map_err(|error| PlayerError::side_effect(SideEffect::Connect, error))?;
        if let Err(error) = self.initialize_connection().await {
            let _ = self.playback.disconnect().await;
            return Err(error);
        }

        let was_idle = self.model.is_idle_connected();
        self.model.voice_channel_id = Some(channel_id);
        self.model.state = PlayerState::IdleConnected;
        self.finish_change(was_idle, next_revision).await
    }

    async fn initialize_connection(&mut self) -> Result<(), PlayerError> {
        self.playback
            .set_volume(self.model.volume)
            .await
            .map_err(|error| PlayerError::side_effect(SideEffect::SetVolume, error))?;
        if let Some(preset) = self.model.hrir_preset.as_ref() {
            self.playback
                .set_hrir(preset)
                .await
                .map_err(|error| PlayerError::side_effect(SideEffect::SetHrir, error))?;
        }
        self.playback
            .set_spatial_audio(self.model.spatial_audio_enabled)
            .await
            .map_err(|error| PlayerError::side_effect(SideEffect::SetSpatialAudio, error))
    }

    pub(super) async fn enqueue(
        &mut self,
        track: QueueTrack,
        expected_revision: StateRevision,
    ) -> Result<PlayerSnapshot, PlayerError> {
        self.check_revision(expected_revision)?;
        if self.model.voice_channel_id.is_none() {
            return Err(PlayerError::NotConnected);
        }
        if !track.has_valid_public_title() {
            return Err(PlayerError::InvalidTrack { field: "title" });
        }
        if self.model.contains_track(track.track_id) {
            return Err(PlayerError::DuplicateTrack {
                track_id: track.track_id,
            });
        }
        if self.model.current.is_some() && self.model.queue.len() >= self.config.max_queued_tracks()
        {
            return Err(PlayerError::QueueFull {
                capacity: self.config.max_queued_tracks(),
            });
        }

        let next_revision = self.next_revision()?;
        let was_idle = self.model.is_idle_connected();
        if self.model.current.is_none() {
            let generation = self.next_playback_generation()?;
            self.playback
                .play(&track, generation)
                .await
                .map_err(|error| PlayerError::side_effect(SideEffect::Play, error))?;
            self.model.current = Some(ActiveTrack::playing(track, generation));
            self.model.state = PlayerState::Playing;
        } else if self.model.shuffle_enabled {
            let seed = seed_from_uuid(track.track_id);
            insert_shuffled(&mut self.model.queue, track, seed);
        } else {
            self.model.queue.push_back(track);
        }
        self.finish_change(was_idle, next_revision).await
    }

    pub(super) async fn enqueue_batch(
        &mut self,
        mut tracks: Vec<QueueTrack>,
        expected_revision: StateRevision,
    ) -> Result<PlayerSnapshot, PlayerError> {
        self.check_revision(expected_revision)?;
        if tracks.is_empty() {
            return Ok(self.current_snapshot());
        }
        if self.model.voice_channel_id.is_none() {
            return Err(PlayerError::NotConnected);
        }

        let starts_immediately = self.model.current.is_none();
        let additional_queued = tracks.len().saturating_sub(usize::from(starts_immediately));
        if self
            .model
            .queue
            .len()
            .checked_add(additional_queued)
            .is_none_or(|total| total > self.config.max_queued_tracks())
        {
            return Err(PlayerError::QueueFull {
                capacity: self.config.max_queued_tracks(),
            });
        }

        // Reject by the configured queue bound before allocating duplicate
        // bookkeeping. `enqueue_batch` is public, so callers outside the
        // Discord adapter must not be able to make the guild actor allocate or
        // scan an arbitrarily large collection on its serialized hot path.
        let mut identities = HashSet::with_capacity(tracks.len());
        for track in &tracks {
            if !track.has_valid_public_title() {
                return Err(PlayerError::InvalidTrack { field: "title" });
            }
            if self.model.contains_track(track.track_id) || !identities.insert(track.track_id) {
                return Err(PlayerError::DuplicateTrack {
                    track_id: track.track_id,
                });
            }
        }

        let next_revision = self.next_revision()?;
        let was_idle = self.model.is_idle_connected();
        if starts_immediately {
            let first = tracks.remove(0);
            let generation = self.next_playback_generation()?;
            self.playback
                .play(&first, generation)
                .await
                .map_err(|error| PlayerError::side_effect(SideEffect::Play, error))?;
            self.model.current = Some(ActiveTrack::playing(first, generation));
            self.model.state = PlayerState::Playing;
        }
        for track in tracks {
            if self.model.shuffle_enabled {
                let seed = seed_from_uuid(track.track_id);
                insert_shuffled(&mut self.model.queue, track, seed);
            } else {
                self.model.queue.push_back(track);
            }
        }
        self.finish_change(was_idle, next_revision).await
    }

    pub(super) async fn remove_queued(
        &mut self,
        track_id: Uuid,
    ) -> Result<PlayerSnapshot, PlayerError> {
        let Some(index) = self
            .model
            .queue
            .iter()
            .position(|track| track.track_id == track_id)
        else {
            return Err(CommandValidationError::QueuedTrackNotFound { track_id }.into());
        };
        let next_revision = self.next_revision()?;
        let was_idle = self.model.is_idle_connected();
        let removed = self.model.queue.remove(index);
        debug_assert!(removed.is_some(), "the queued index was found above");
        drop(removed);
        self.finish_change(was_idle, next_revision).await
    }

    pub(super) async fn move_queued(
        &mut self,
        track_id: Uuid,
        before_track_id: Option<Uuid>,
    ) -> Result<PlayerSnapshot, PlayerError> {
        let Some(source_index) = self
            .model
            .queue
            .iter()
            .position(|track| track.track_id == track_id)
        else {
            return Err(CommandValidationError::QueuedTrackNotFound { track_id }.into());
        };
        let destination_index = match before_track_id {
            Some(before_track_id) if before_track_id == track_id => source_index,
            Some(before_track_id) => {
                let Some(target_index) = self
                    .model
                    .queue
                    .iter()
                    .position(|track| track.track_id == before_track_id)
                else {
                    return Err(CommandValidationError::QueuedTrackNotFound {
                        track_id: before_track_id,
                    }
                    .into());
                };
                if target_index > source_index {
                    target_index - 1
                } else {
                    target_index
                }
            }
            None => self.model.queue.len().saturating_sub(1),
        };
        if destination_index == source_index {
            return Ok(self.current_snapshot());
        }

        let next_revision = self.next_revision()?;
        let was_idle = self.model.is_idle_connected();
        let moved = self
            .model
            .queue
            .remove(source_index)
            .expect("the queued source index was found above");
        self.model.queue.insert(destination_index, moved);
        self.finish_change(was_idle, next_revision).await
    }
}
