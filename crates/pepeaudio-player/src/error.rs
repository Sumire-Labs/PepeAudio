use pepeaudio_core::{ChannelId, CommandValidationError, StateRevision};
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SideEffect {
    Connect,
    Play,
    Pause,
    Resume,
    Stop,
    Seek,
    SetVolume,
    SetHrir,
    SetSpatialAudio,
    Disconnect,
}

impl std::fmt::Display for SideEffect {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

#[derive(Debug, Error)]
pub enum PlayerError {
    #[error("the guild player actor has stopped")]
    ActorStopped,
    #[error("expected revision {expected}, but the authoritative revision is {actual}")]
    RevisionConflict { expected: u64, actual: u64 },
    #[error("the guild player is not connected to voice")]
    NotConnected,
    #[error("the guild player is connected to channel {connected}, not {requested}")]
    VoiceChannelMismatch {
        connected: ChannelId,
        requested: ChannelId,
    },
    #[error("the playback queue is full at {capacity} upcoming tracks")]
    QueueFull { capacity: usize },
    #[error("track {track_id} is already current or queued")]
    DuplicateTrack { track_id: Uuid },
    #[error("track has an invalid public {field}")]
    InvalidTrack { field: &'static str },
    #[error(transparent)]
    InvalidCommand(#[from] CommandValidationError),
    #[error("playback side effect {operation} failed: {message}")]
    SideEffect {
        operation: SideEffect,
        message: String,
    },
    #[error("state revision {revision:?} is exhausted")]
    RevisionExhausted { revision: StateRevision },
    #[error("idle timer generation is exhausted")]
    IdleGenerationExhausted,
    #[error("the guild player task failed: {message}")]
    TaskFailed { message: String },
}

impl PlayerError {
    pub(crate) fn side_effect(operation: SideEffect, error: impl std::fmt::Display) -> Self {
        Self::SideEffect {
            operation,
            message: error.to_string(),
        }
    }
}
