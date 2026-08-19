use pepeaudio_core::GuildId;
use pepeaudio_player::PlaybackIdentity;

/// Why Songbird disposed an unsuppressed active track.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlaybackEndReason {
    Natural,
    WorkerFailed,
    SongbirdEnded,
    SongbirdError,
}

/// Stable failure class safe to publish to orchestration code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkerFailure {
    Decoder,
    Audio,
    Output,
    Task,
}

/// Best-effort event emitted by a guild playback pipeline.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlaybackEvent {
    WorkerFailed {
        guild_id: GuildId,
        identity: PlaybackIdentity,
        failure: WorkerFailure,
    },
    TrackEnded {
        guild_id: GuildId,
        identity: PlaybackIdentity,
        reason: PlaybackEndReason,
    },
}
