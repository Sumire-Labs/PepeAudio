use pepeaudio_core::{
    GuildId, PlayerSnapshot, PlayerState, RepeatMode, StateRevision, UnixTimeMillis, Volume,
};

pub(crate) fn initial_snapshot(guild_id: GuildId, observed_at: UnixTimeMillis) -> PlayerSnapshot {
    PlayerSnapshot {
        guild_id,
        voice_channel_id: None,
        revision: StateRevision::INITIAL,
        state: PlayerState::Disconnected,
        current_track: None,
        queued_tracks: 0,
        upcoming_tracks: Vec::new(),
        has_previous_track: false,
        volume: Volume::DEFAULT,
        repeat_mode: RepeatMode::Off,
        shuffle_enabled: false,
        hrir_preset: None,
        spatial_audio_enabled: false,
        observed_at,
    }
}
