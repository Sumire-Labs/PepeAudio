use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    CommandValidationError, HrirPresetId, PlayerSnapshot, PlayerState, RepeatMode, Volume,
};

/// A player mutation understood by a guild's owning audio worker.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PlayerCommand {
    /// Resolves and enqueues a URL or song-title search outside the realtime actor.
    EnqueueMedia {
        input: String,
    },
    Play,
    Pause,
    /// Stop playback and clear the queue.
    Stop,
    /// Stop the current track and advance to the next queue entry.
    Skip,
    /// Return to the previous history entry.
    Previous,
    /// Remove one upcoming queue entry without interrupting the current track.
    RemoveQueued {
        track_id: Uuid,
    },
    /// Move one upcoming queue entry before another, or to the end.
    MoveQueued {
        track_id: Uuid,
        /// Stable identity to insert before. `None` moves the track to the end.
        before_track_id: Option<Uuid>,
    },
    Seek {
        position_ms: u64,
    },
    SetVolume {
        volume: Volume,
    },
    SetRepeat {
        mode: RepeatMode,
    },
    SetShuffle {
        enabled: bool,
    },
    SetHrir {
        preset: HrirPresetId,
    },
    SetSpatialAudio {
        enabled: bool,
    },
    /// Disconnect from voice and release the guild's live audio resources.
    Disconnect,
}

impl PlayerCommand {
    pub(crate) fn validate_against(
        &self,
        snapshot: &PlayerSnapshot,
    ) -> Result<(), CommandValidationError> {
        match self {
            Self::EnqueueMedia { input }
                if input.trim().is_empty()
                    || input.len() > 4_096
                    || input.chars().any(char::is_control) =>
            {
                Err(CommandValidationError::InvalidMediaInput)
            }
            Self::Play if snapshot.state != PlayerState::Paused => {
                Err(CommandValidationError::UnavailableInState {
                    command: "play",
                    state: snapshot.state,
                })
            }
            Self::Pause if snapshot.state != PlayerState::Playing => {
                Err(CommandValidationError::UnavailableInState {
                    command: "pause",
                    state: snapshot.state,
                })
            }
            Self::Stop if snapshot.state == PlayerState::Disconnected => {
                Err(CommandValidationError::UnavailableInState {
                    command: "stop",
                    state: snapshot.state,
                })
            }
            Self::Skip if snapshot.current_track.is_none() => {
                Err(CommandValidationError::NoCurrentTrack { command: "skip" })
            }
            Self::Previous if !snapshot.has_previous_track => {
                Err(CommandValidationError::NoPreviousTrack)
            }
            Self::RemoveQueued { track_id }
                if !snapshot
                    .upcoming_tracks
                    .iter()
                    .any(|track| track.track_id == *track_id) =>
            {
                Err(CommandValidationError::QueuedTrackNotFound {
                    track_id: *track_id,
                })
            }
            Self::MoveQueued {
                track_id,
                before_track_id,
            } => {
                if !snapshot
                    .upcoming_tracks
                    .iter()
                    .any(|track| track.track_id == *track_id)
                {
                    return Err(CommandValidationError::QueuedTrackNotFound {
                        track_id: *track_id,
                    });
                }
                if let Some(before_track_id) = before_track_id
                    && !snapshot
                        .upcoming_tracks
                        .iter()
                        .any(|track| track.track_id == *before_track_id)
                {
                    return Err(CommandValidationError::QueuedTrackNotFound {
                        track_id: *before_track_id,
                    });
                }
                Ok(())
            }
            Self::Seek { position_ms } => {
                let track = snapshot
                    .current_track
                    .as_ref()
                    .ok_or(CommandValidationError::NoCurrentTrack { command: "seek" })?;
                if !track.seekable {
                    return Err(CommandValidationError::TrackNotSeekable);
                }
                if let Some(duration_ms) = track.duration_ms
                    && *position_ms > duration_ms
                {
                    return Err(CommandValidationError::SeekPastEnd {
                        requested_ms: *position_ms,
                        duration_ms,
                    });
                }
                Ok(())
            }
            Self::EnqueueMedia { .. }
            | Self::Play
            | Self::Pause
            | Self::Stop
            | Self::Skip
            | Self::Previous
            | Self::RemoveQueued { .. }
            | Self::SetVolume { .. }
            | Self::SetRepeat { .. }
            | Self::SetShuffle { .. }
            | Self::SetHrir { .. }
            | Self::SetSpatialAudio { .. }
            | Self::Disconnect => Ok(()),
        }
    }
}
