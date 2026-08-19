use std::fmt;

use crate::{GuildId, PlayerState, StateRevision, UnixTimeMillis};
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommandValidationError {
    GuildMismatch {
        command_guild_id: GuildId,
        snapshot_guild_id: GuildId,
    },
    DeadlineExpired {
        deadline: UnixTimeMillis,
        now: UnixTimeMillis,
    },
    RevisionConflict {
        expected: StateRevision,
        actual: StateRevision,
    },
    UnavailableInState {
        command: &'static str,
        state: PlayerState,
    },
    NoCurrentTrack {
        command: &'static str,
    },
    NoPreviousTrack,
    QueuedTrackNotFound {
        track_id: Uuid,
    },
    TrackNotSeekable,
    SeekPastEnd {
        requested_ms: u64,
        duration_ms: u64,
    },
}

impl fmt::Display for CommandValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GuildMismatch {
                command_guild_id,
                snapshot_guild_id,
            } => write!(
                formatter,
                "command guild {command_guild_id} does not match snapshot guild {snapshot_guild_id}"
            ),
            Self::DeadlineExpired { deadline, now } => write!(
                formatter,
                "command deadline {} expired at validation time {}",
                deadline.get(),
                now.get()
            ),
            Self::RevisionConflict { expected, actual } => write!(
                formatter,
                "command expected revision {}, but current revision is {}",
                expected.get(),
                actual.get()
            ),
            Self::UnavailableInState { command, state } => {
                write!(
                    formatter,
                    "command {command} is unavailable in state {state:?}"
                )
            }
            Self::NoCurrentTrack { command } => {
                write!(formatter, "command {command} requires a current track")
            }
            Self::NoPreviousTrack => formatter.write_str("there is no previous track"),
            Self::QueuedTrackNotFound { track_id } => {
                write!(formatter, "upcoming track {track_id} was not found")
            }
            Self::TrackNotSeekable => formatter.write_str("the current track is not seekable"),
            Self::SeekPastEnd {
                requested_ms,
                duration_ms,
            } => write!(
                formatter,
                "seek position {requested_ms} ms exceeds duration {duration_ms} ms"
            ),
        }
    }
}

impl std::error::Error for CommandValidationError {}
