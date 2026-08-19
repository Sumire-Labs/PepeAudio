use pepeaudio_core::{PlayerCommand, PlayerSnapshot, PlayerState};
use uuid::Uuid;

pub(super) fn apply(snapshot: &mut PlayerSnapshot, command: &PlayerCommand) {
    match command {
        PlayerCommand::Play => snapshot.state = PlayerState::Playing,
        PlayerCommand::Pause => snapshot.state = PlayerState::Paused,
        PlayerCommand::Stop => {
            snapshot.state = PlayerState::IdleConnected;
            snapshot.current_track = None;
            snapshot.has_previous_track = false;
            clear_upcoming(snapshot);
        }
        PlayerCommand::Skip | PlayerCommand::Previous => {
            snapshot.current_track = None;
            snapshot.state = PlayerState::Loading;
            if !snapshot.upcoming_tracks.is_empty() {
                snapshot.upcoming_tracks.remove(0);
            }
            sync_queued_count(snapshot);
        }
        PlayerCommand::RemoveQueued { track_id } => {
            snapshot
                .upcoming_tracks
                .retain(|track| track.track_id != *track_id);
            sync_queued_count(snapshot);
        }
        PlayerCommand::MoveQueued {
            track_id,
            before_track_id,
        } => move_queued(snapshot, *track_id, *before_track_id),
        PlayerCommand::Seek { position_ms } => {
            if let Some(track) = &mut snapshot.current_track {
                track.position_ms = *position_ms;
            }
        }
        PlayerCommand::SetVolume { volume } => snapshot.volume = *volume,
        PlayerCommand::SetRepeat { mode } => snapshot.repeat_mode = *mode,
        PlayerCommand::SetShuffle { enabled } => snapshot.shuffle_enabled = *enabled,
        PlayerCommand::SetHrir { preset } => snapshot.hrir_preset = Some(preset.clone()),
        PlayerCommand::SetSpatialAudio { enabled } => {
            snapshot.spatial_audio_enabled = *enabled;
        }
        PlayerCommand::Disconnect => {
            snapshot.state = PlayerState::Disconnected;
            snapshot.voice_channel_id = None;
            snapshot.current_track = None;
            snapshot.has_previous_track = false;
            clear_upcoming(snapshot);
        }
    }
}

fn clear_upcoming(snapshot: &mut PlayerSnapshot) {
    snapshot.upcoming_tracks.clear();
    snapshot.queued_tracks = 0;
}

fn sync_queued_count(snapshot: &mut PlayerSnapshot) {
    snapshot.queued_tracks = u32::try_from(snapshot.upcoming_tracks.len()).unwrap_or(u32::MAX);
}

fn move_queued(snapshot: &mut PlayerSnapshot, track_id: Uuid, before_track_id: Option<Uuid>) {
    if before_track_id == Some(track_id) {
        return;
    }
    let Some(source_index) = snapshot
        .upcoming_tracks
        .iter()
        .position(|track| track.track_id == track_id)
    else {
        return;
    };
    let moved = snapshot.upcoming_tracks.remove(source_index);
    let destination_index = before_track_id
        .and_then(|target| {
            snapshot
                .upcoming_tracks
                .iter()
                .position(|track| track.track_id == target)
        })
        .unwrap_or(snapshot.upcoming_tracks.len());
    snapshot.upcoming_tracks.insert(destination_index, moved);
}

#[cfg(test)]
mod tests {
    use pepeaudio_core::{
        ChannelId, GuildId, PlayerCommand, PlayerSnapshot, PlayerState, RepeatMode, StateRevision,
        TrackSnapshot, UnixTimeMillis, Volume,
    };
    use uuid::Uuid;

    use super::apply;

    #[test]
    fn queue_mutations_keep_the_count_and_items_in_sync() {
        let mut removed = snapshot();
        apply(
            &mut removed,
            &PlayerCommand::RemoveQueued {
                track_id: Uuid::from_u128(2),
            },
        );
        assert_eq!(removed.queued_tracks, 2);
        assert_eq!(ids(&removed), vec![1, 3]);

        let mut reordered = snapshot();
        apply(
            &mut reordered,
            &PlayerCommand::MoveQueued {
                track_id: Uuid::from_u128(3),
                before_track_id: Some(Uuid::from_u128(1)),
            },
        );
        assert_eq!(reordered.queued_tracks, 3);
        assert_eq!(ids(&reordered), vec![3, 1, 2]);

        for command in [PlayerCommand::Skip, PlayerCommand::Previous] {
            let mut advanced = snapshot();
            apply(&mut advanced, &command);
            assert_eq!(advanced.queued_tracks, 2);
            assert_eq!(ids(&advanced), vec![2, 3]);
        }
    }

    #[test]
    fn stop_and_disconnect_clear_every_queue_view() {
        for command in [PlayerCommand::Stop, PlayerCommand::Disconnect] {
            let mut cleared = snapshot();
            apply(&mut cleared, &command);
            assert_eq!(cleared.queued_tracks, 0);
            assert!(cleared.upcoming_tracks.is_empty());
            assert!(!cleared.has_previous_track);
        }
    }

    fn snapshot() -> PlayerSnapshot {
        let upcoming_tracks = [1_u128, 2, 3].into_iter().map(track).collect();
        PlayerSnapshot {
            guild_id: GuildId::new(1).expect("guild"),
            voice_channel_id: Some(ChannelId::new(2).expect("channel")),
            revision: StateRevision::new(4),
            state: PlayerState::Playing,
            current_track: Some(track(4)),
            queued_tracks: 3,
            upcoming_tracks,
            has_previous_track: true,
            volume: Volume::DEFAULT,
            repeat_mode: RepeatMode::Off,
            shuffle_enabled: false,
            hrir_preset: None,
            spatial_audio_enabled: false,
            observed_at: UnixTimeMillis::new(1),
        }
    }

    fn track(id: u128) -> TrackSnapshot {
        TrackSnapshot {
            track_id: Uuid::from_u128(id),
            title: format!("track {id}"),
            artist: None,
            album: None,
            provenance: None,
            requester_user_id: None,
            duration_ms: Some(1_000),
            position_ms: 0,
            seekable: true,
        }
    }

    fn ids(snapshot: &PlayerSnapshot) -> Vec<u128> {
        snapshot
            .upcoming_tracks
            .iter()
            .map(|track| track.track_id.as_u128())
            .collect()
    }
}
