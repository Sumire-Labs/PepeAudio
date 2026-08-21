use pepeaudio_core::{
    CommandResult, CommandResultCode, CommandValidationError, GuildId, StateRevision,
};
use pepeaudio_player::PlayerError;
use uuid::Uuid;

pub(crate) fn applied(
    command_id: Uuid,
    guild_id: GuildId,
    snapshot_guild_id: GuildId,
    revision: StateRevision,
) -> CommandResult {
    if snapshot_guild_id == guild_id {
        CommandResult::applied(command_id, guild_id, revision)
    } else {
        CommandResult::rejected(
            command_id,
            guild_id,
            CommandResultCode::InvalidPlayerState,
            None,
        )
    }
}

pub(crate) fn rejected(
    command_id: Uuid,
    guild_id: GuildId,
    error: &PlayerError,
) -> Option<CommandResult> {
    let (code, current_revision) = match error {
        PlayerError::RevisionConflict { actual, .. } => (
            CommandResultCode::RevisionConflict,
            Some(StateRevision::new(*actual)),
        ),
        PlayerError::NotConnected => (CommandResultCode::NotConnected, None),
        PlayerError::VoiceChannelMismatch { .. } => (CommandResultCode::VoiceChannelMismatch, None),
        PlayerError::QueueFull { .. } => (CommandResultCode::QueueFull, None),
        PlayerError::DuplicateTrack { .. } => (CommandResultCode::DuplicateTrack, None),
        PlayerError::InvalidCommand(error) => validation_rejection(error),
        PlayerError::RevisionExhausted { .. } | PlayerError::IdleGenerationExhausted => {
            (CommandResultCode::StateExhausted, None)
        }
        PlayerError::ActorStopped
        | PlayerError::InvalidTrack { .. }
        | PlayerError::SideEffect { .. }
        | PlayerError::TaskFailed { .. } => return None,
    };
    Some(CommandResult::rejected(
        command_id,
        guild_id,
        code,
        current_revision,
    ))
}

fn validation_rejection(
    error: &CommandValidationError,
) -> (CommandResultCode, Option<StateRevision>) {
    match error {
        CommandValidationError::RevisionConflict { actual, .. } => {
            (CommandResultCode::RevisionConflict, Some(*actual))
        }
        CommandValidationError::DeadlineExpired { .. } => {
            (CommandResultCode::DeadlineExpired, None)
        }
        CommandValidationError::NoCurrentTrack { .. } => (CommandResultCode::NoCurrentTrack, None),
        CommandValidationError::NoPreviousTrack => (CommandResultCode::NoPreviousTrack, None),
        CommandValidationError::QueuedTrackNotFound { .. } => {
            (CommandResultCode::QueuedTrackNotFound, None)
        }
        CommandValidationError::TrackNotSeekable => (CommandResultCode::TrackNotSeekable, None),
        CommandValidationError::SeekPastEnd { .. } => (CommandResultCode::SeekPastEnd, None),
        CommandValidationError::InvalidMediaInput => (CommandResultCode::InvalidMediaInput, None),
        CommandValidationError::GuildMismatch { .. }
        | CommandValidationError::UnavailableInState { .. } => {
            (CommandResultCode::InvalidPlayerState, None)
        }
    }
}

#[cfg(test)]
mod tests {
    use pepeaudio_core::{CommandResultCode, CommandResultStatus, GuildId};
    use pepeaudio_player::{PlayerError, SideEffect};
    use uuid::Uuid;

    use super::rejected;

    #[test]
    fn adapter_failures_remain_non_terminal() {
        assert!(
            rejected(
                Uuid::nil(),
                GuildId::new(1).expect("guild"),
                &PlayerError::SideEffect {
                    operation: SideEffect::Play,
                    message: "private adapter detail".into(),
                }
            )
            .is_none()
        );
    }

    #[test]
    fn rejection_contains_only_a_stable_code() {
        let result = rejected(
            Uuid::nil(),
            GuildId::new(1).expect("guild"),
            &PlayerError::QueueFull { capacity: 99 },
        )
        .expect("terminal rejection");
        assert_eq!(
            result.status,
            CommandResultStatus::Rejected {
                code: CommandResultCode::QueueFull,
                current_revision: None,
            }
        );
    }
}
