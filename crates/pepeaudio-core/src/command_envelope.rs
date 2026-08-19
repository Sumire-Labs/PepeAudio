use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    CommandValidationError, GuildId, PlayerCommand, PlayerSnapshot, StateRevision, UnixTimeMillis,
    UserId,
};

/// A transport-neutral command with routing and concurrency metadata.
///
/// `command_id` identifies one transport attempt, while `idempotency_key`
/// identifies the logical mutation and must be reused by callers when retrying.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CommandEnvelope {
    pub command_id: Uuid,
    pub idempotency_key: Uuid,
    pub guild_id: GuildId,
    /// User who initiated the mutation, or `None` for a trusted system action.
    pub actor_user_id: Option<UserId>,
    pub expected_revision: StateRevision,
    /// Latest instant at which a worker may begin applying the command.
    pub deadline: UnixTimeMillis,
    pub command: PlayerCommand,
}

impl CommandEnvelope {
    /// Creates a command with fresh command and idempotency UUIDs.
    #[must_use]
    pub fn new(
        guild_id: GuildId,
        actor_user_id: Option<UserId>,
        expected_revision: StateRevision,
        deadline: UnixTimeMillis,
        command: PlayerCommand,
    ) -> Self {
        Self {
            command_id: Uuid::new_v4(),
            idempotency_key: Uuid::new_v4(),
            guild_id,
            actor_user_id,
            expected_revision,
            deadline,
            command,
        }
    }

    /// This method does not reserve or increment the revision. The owning worker
    /// must perform validation and mutation atomically, then publish a snapshot
    /// with the next revision.
    ///
    /// # Errors
    ///
    /// Returns [`CommandValidationError`] for invalid routing, timing, revision,
    /// or player-state semantics.
    pub fn validate_against(
        &self,
        snapshot: &PlayerSnapshot,
        now: UnixTimeMillis,
    ) -> Result<(), CommandValidationError> {
        if self.guild_id != snapshot.guild_id {
            return Err(CommandValidationError::GuildMismatch {
                command_guild_id: self.guild_id,
                snapshot_guild_id: snapshot.guild_id,
            });
        }
        if now >= self.deadline {
            return Err(CommandValidationError::DeadlineExpired {
                deadline: self.deadline,
                now,
            });
        }
        if self.expected_revision != snapshot.revision {
            return Err(CommandValidationError::RevisionConflict {
                expected: self.expected_revision,
                actual: snapshot.revision,
            });
        }

        self.command.validate_against(snapshot)
    }
}
