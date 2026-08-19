use pepeaudio_core::{
    CommandEnvelope, HrirPresetId, PlayerCommand, PlayerSnapshot, RepeatMode, UnixTimeMillis,
    UserId, Volume,
};
use thiserror::Error;

use crate::{ComponentAction, DecodedComponentId};

/// User-controlled value accompanying a verified component ID.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InteractionInput {
    Button,
    Select(String),
}

/// Maps a verified component interaction to a core mutation envelope.
///
/// # Errors
///
/// Returns [`InteractionMapError`] for a stale/misrouted ID or an invalid
/// action-specific value.
pub fn map_interaction(
    decoded: DecodedComponentId,
    input: InteractionInput,
    actor_user_id: UserId,
    snapshot: &PlayerSnapshot,
    deadline: UnixTimeMillis,
) -> Result<CommandEnvelope, InteractionMapError> {
    if decoded.guild_id != snapshot.guild_id {
        return Err(InteractionMapError::GuildMismatch);
    }
    if decoded.revision != snapshot.revision {
        return Err(InteractionMapError::StaleRevision);
    }
    let command = map_action(decoded.action, input, snapshot)?;
    Ok(CommandEnvelope::new(
        decoded.guild_id,
        Some(actor_user_id),
        decoded.revision,
        deadline,
        command,
    ))
}

fn map_action(
    action: ComponentAction,
    input: InteractionInput,
    snapshot: &PlayerSnapshot,
) -> Result<PlayerCommand, InteractionMapError> {
    match (action, input) {
        (ComponentAction::PlayPause, InteractionInput::Button) => {
            if snapshot.state == pepeaudio_core::PlayerState::Playing {
                Ok(PlayerCommand::Pause)
            } else {
                Ok(PlayerCommand::Play)
            }
        }
        (ComponentAction::Previous, InteractionInput::Button) => Ok(PlayerCommand::Previous),
        (ComponentAction::Skip, InteractionInput::Button) => Ok(PlayerCommand::Skip),
        (ComponentAction::Stop, InteractionInput::Button) => Ok(PlayerCommand::Stop),
        (ComponentAction::Repeat, InteractionInput::Button) => Ok(PlayerCommand::SetRepeat {
            mode: match snapshot.repeat_mode {
                RepeatMode::Off => RepeatMode::Track,
                RepeatMode::Track => RepeatMode::Queue,
                RepeatMode::Queue => RepeatMode::Off,
            },
        }),
        (ComponentAction::Shuffle, InteractionInput::Button) => Ok(PlayerCommand::SetShuffle {
            enabled: !snapshot.shuffle_enabled,
        }),
        (ComponentAction::Spatial, InteractionInput::Button) => {
            Ok(PlayerCommand::SetSpatialAudio {
                enabled: !snapshot.spatial_audio_enabled,
            })
        }
        (ComponentAction::Volume, InteractionInput::Select(value)) => {
            let value = value
                .parse::<u8>()
                .map_err(|_| InteractionMapError::InvalidValue)?;
            let volume = Volume::new(value).map_err(|_| InteractionMapError::InvalidValue)?;
            Ok(PlayerCommand::SetVolume { volume })
        }
        (ComponentAction::Hrir, InteractionInput::Select(value)) => {
            let preset = HrirPresetId::new(value).map_err(|_| InteractionMapError::InvalidValue)?;
            Ok(PlayerCommand::SetHrir { preset })
        }
        _ => Err(InteractionMapError::InputKindMismatch),
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum InteractionMapError {
    #[error("component belongs to another guild")]
    GuildMismatch,
    #[error("component was rendered from an obsolete player revision")]
    StaleRevision,
    #[error("button/select value does not match the encoded action")]
    InputKindMismatch,
    #[error("component value is invalid")]
    InvalidValue,
}

#[cfg(test)]
mod tests {
    use pepeaudio_core::{
        GuildId, PlayerCommand, PlayerSnapshot, PlayerState, RepeatMode, StateRevision,
        UnixTimeMillis, UserId, Volume,
    };

    use super::{InteractionInput, map_interaction};
    use crate::{ComponentAction, DecodedComponentId};

    fn snapshot() -> PlayerSnapshot {
        PlayerSnapshot {
            guild_id: GuildId::new(1).expect("guild"),
            voice_channel_id: None,
            revision: StateRevision::new(2),
            state: PlayerState::Playing,
            current_track: None,
            queued_tracks: 0,
            upcoming_tracks: Vec::new(),
            has_previous_track: false,
            volume: Volume::DEFAULT,
            repeat_mode: RepeatMode::Off,
            shuffle_enabled: false,
            hrir_preset: None,
            spatial_audio_enabled: false,
            observed_at: UnixTimeMillis::new(0),
        }
    }

    #[test]
    fn play_pause_reflects_snapshot_state() {
        let envelope = map_interaction(
            DecodedComponentId {
                action: ComponentAction::PlayPause,
                guild_id: GuildId::new(1).expect("guild"),
                revision: StateRevision::new(2),
            },
            InteractionInput::Button,
            UserId::new(3).expect("user"),
            &snapshot(),
            UnixTimeMillis::new(u64::MAX),
        )
        .expect("maps");
        assert_eq!(envelope.command, PlayerCommand::Pause);
    }
}
