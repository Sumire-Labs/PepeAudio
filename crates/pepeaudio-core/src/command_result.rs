use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{GuildId, StateRevision};

/// Short-lived, guild-scoped status for one command transport attempt.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CommandResult {
    /// Transport attempt returned by the command submission endpoint.
    pub command_id: Uuid,
    /// Guild boundary used to prevent cross-guild result disclosure.
    pub guild_id: GuildId,
    #[serde(flatten)]
    pub status: CommandResultStatus,
}

impl CommandResult {
    /// Written atomically with command enqueueing.
    #[must_use]
    pub const fn pending(command_id: Uuid, guild_id: GuildId) -> Self {
        Self {
            command_id,
            guild_id,
            status: CommandResultStatus::Pending,
        }
    }

    #[must_use]
    pub const fn applied(
        command_id: Uuid,
        guild_id: GuildId,
        resulting_revision: StateRevision,
    ) -> Self {
        Self {
            command_id,
            guild_id,
            status: CommandResultStatus::Applied { resulting_revision },
        }
    }

    #[must_use]
    pub const fn denied(command_id: Uuid, guild_id: GuildId, code: CommandResultCode) -> Self {
        Self {
            command_id,
            guild_id,
            status: CommandResultStatus::Denied { code },
        }
    }

    #[must_use]
    pub const fn rejected(
        command_id: Uuid,
        guild_id: GuildId,
        code: CommandResultCode,
        current_revision: Option<StateRevision>,
    ) -> Self {
        Self {
            command_id,
            guild_id,
            status: CommandResultStatus::Rejected {
                code,
                current_revision,
            },
        }
    }

    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        !matches!(self.status, CommandResultStatus::Pending)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum CommandResultStatus {
    Pending,
    Applied {
        resulting_revision: StateRevision,
    },
    Denied {
        code: CommandResultCode,
    },
    Rejected {
        code: CommandResultCode,
        /// Current revision when a fresh snapshot can resolve the rejection.
        #[serde(skip_serializing_if = "Option::is_none")]
        current_revision: Option<StateRevision>,
    },
}

/// Stable, non-sensitive terminal reason exposed to command submitters.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandResultCode {
    NotAuthorized,
    RevisionConflict,
    DeadlineExpired,
    InvalidPlayerState,
    NoCurrentTrack,
    NoPreviousTrack,
    QueuedTrackNotFound,
    TrackNotSeekable,
    SeekPastEnd,
    NotConnected,
    VoiceChannelMismatch,
    QueueFull,
    DuplicateTrack,
    StateExhausted,
    IdempotencyReplayed,
    ResultExpired,
}

#[cfg(test)]
mod tests {
    use super::{CommandResult, CommandResultCode};
    use crate::{GuildId, StateRevision};
    use uuid::Uuid;

    #[test]
    fn wire_shape_is_flat_and_uses_stable_codes() {
        let result = CommandResult::rejected(
            Uuid::from_u128(1),
            GuildId::new(2).expect("guild"),
            CommandResultCode::RevisionConflict,
            Some(StateRevision::new(9)),
        );

        assert_eq!(
            serde_json::to_value(result).expect("serialize"),
            serde_json::json!({
                "command_id": "00000000-0000-0000-0000-000000000001",
                "guild_id": "2",
                "status": "rejected",
                "code": "revision_conflict",
                "current_revision": 9
            })
        );
    }

    #[test]
    fn only_pending_results_are_non_terminal() {
        let command_id = Uuid::from_u128(1);
        let guild_id = GuildId::new(2).expect("guild");
        assert!(!CommandResult::pending(command_id, guild_id).is_terminal());
        assert!(CommandResult::applied(command_id, guild_id, StateRevision::INITIAL).is_terminal());
    }
}
