use pepeaudio_core::{
    ChannelId, CommandEnvelope, CommandValidationError, GuildId, PlayerCommand, PlayerSnapshot,
    PlayerState, RepeatMode, StateRevision, TrackSnapshot, UnixTimeMillis, UserId, Volume,
};
use uuid::Uuid;

fn snapshot() -> PlayerSnapshot {
    let upcoming_tracks = [1_u128, 2]
        .into_iter()
        .map(|value| TrackSnapshot {
            track_id: Uuid::from_u128(value),
            title: format!("Queued track {value}"),
            artist: None,
            album: None,
            provenance: None,
            requester_user_id: Some(UserId::new(20).expect("valid user")),
            duration_ms: Some(120_000),
            position_ms: 0,
            seekable: true,
        })
        .collect();
    PlayerSnapshot {
        guild_id: GuildId::new(10).expect("valid guild"),
        voice_channel_id: Some(ChannelId::new(30).expect("valid channel")),
        revision: StateRevision::new(7),
        state: PlayerState::Playing,
        current_track: Some(TrackSnapshot {
            track_id: Uuid::nil(),
            title: "Test track".into(),
            artist: None,
            album: None,
            provenance: None,
            requester_user_id: Some(UserId::new(20).expect("valid user")),
            duration_ms: Some(180_000),
            position_ms: 30_000,
            seekable: true,
        }),
        queued_tracks: 2,
        upcoming_tracks,
        has_previous_track: true,
        volume: Volume::DEFAULT,
        repeat_mode: RepeatMode::Off,
        shuffle_enabled: false,
        hrir_preset: None,
        spatial_audio_enabled: false,
        observed_at: UnixTimeMillis::new(1_000),
    }
}

fn command(player_command: PlayerCommand) -> CommandEnvelope {
    CommandEnvelope {
        command_id: Uuid::new_v4(),
        idempotency_key: Uuid::new_v4(),
        guild_id: GuildId::new(10).expect("valid guild"),
        actor_user_id: Some(UserId::new(20).expect("valid user")),
        expected_revision: StateRevision::new(7),
        deadline: UnixTimeMillis::new(2_000),
        command: player_command,
    }
}

#[test]
fn optimistic_revision_must_match_exactly() {
    let mut stale = command(PlayerCommand::Pause);
    stale.expected_revision = StateRevision::new(6);

    assert_eq!(
        stale.validate_against(&snapshot(), UnixTimeMillis::new(1_500)),
        Err(CommandValidationError::RevisionConflict {
            expected: StateRevision::new(6),
            actual: StateRevision::new(7),
        })
    );
}

#[test]
fn deadline_is_exclusive() {
    let envelope = command(PlayerCommand::Pause);

    assert!(matches!(
        envelope.validate_against(&snapshot(), UnixTimeMillis::new(2_000)),
        Err(CommandValidationError::DeadlineExpired { .. })
    ));
}

#[test]
fn seek_cannot_exceed_known_duration() {
    let envelope = command(PlayerCommand::Seek {
        position_ms: 180_001,
    });

    assert_eq!(
        envelope.validate_against(&snapshot(), UnixTimeMillis::new(1_500)),
        Err(CommandValidationError::SeekPastEnd {
            requested_ms: 180_001,
            duration_ms: 180_000,
        })
    );
}

#[test]
fn seek_requires_a_seekable_current_track() {
    let mut live = snapshot();
    let track = live.current_track.as_mut().expect("current track");
    track.duration_ms = None;
    track.seekable = false;

    assert_eq!(
        command(PlayerCommand::Seek { position_ms: 1 })
            .validate_against(&live, UnixTimeMillis::new(1_500)),
        Err(CommandValidationError::TrackNotSeekable)
    );
}

#[test]
fn valid_command_passes_all_envelope_checks() {
    assert_eq!(
        command(PlayerCommand::Pause).validate_against(&snapshot(), UnixTimeMillis::new(1_500)),
        Ok(())
    );
}

#[test]
fn queued_track_removal_requires_an_upcoming_identity() {
    let existing = Uuid::from_u128(2);
    assert_eq!(
        command(PlayerCommand::RemoveQueued { track_id: existing })
            .validate_against(&snapshot(), UnixTimeMillis::new(1_500)),
        Ok(())
    );

    let missing = Uuid::from_u128(3);
    assert_eq!(
        command(PlayerCommand::RemoveQueued { track_id: missing })
            .validate_against(&snapshot(), UnixTimeMillis::new(1_500)),
        Err(CommandValidationError::QueuedTrackNotFound { track_id: missing })
    );
}

#[test]
fn queued_track_move_requires_both_upcoming_identities() {
    let first = Uuid::from_u128(1);
    let second = Uuid::from_u128(2);
    let missing = Uuid::from_u128(3);

    for player_command in [
        PlayerCommand::MoveQueued {
            track_id: second,
            before_track_id: Some(first),
        },
        PlayerCommand::MoveQueued {
            track_id: first,
            before_track_id: None,
        },
    ] {
        assert_eq!(
            command(player_command).validate_against(&snapshot(), UnixTimeMillis::new(1_500)),
            Ok(())
        );
    }

    for (player_command, expected_missing) in [
        (
            PlayerCommand::MoveQueued {
                track_id: missing,
                before_track_id: Some(first),
            },
            missing,
        ),
        (
            PlayerCommand::MoveQueued {
                track_id: first,
                before_track_id: Some(missing),
            },
            missing,
        ),
    ] {
        assert_eq!(
            command(player_command).validate_against(&snapshot(), UnixTimeMillis::new(1_500)),
            Err(CommandValidationError::QueuedTrackNotFound {
                track_id: expected_missing
            })
        );
    }
}
