//! Transport-independent domain types for `PepeAudio`.
//!
//! This crate deliberately contains no Discord, database, cache, or audio-driver
//! dependencies. Gateway workers and the web API can therefore share the same
//! validated commands and player snapshots.

mod clock;
mod command;
mod command_envelope;
mod command_error;
mod command_rate_limit;
mod command_result;
mod hrir_id;
mod id;
mod player;
mod shard;
mod track_provenance;
mod volume;

pub use clock::{StateRevision, UnixTimeMillis};
pub use command::PlayerCommand;
pub use command_envelope::CommandEnvelope;
pub use command_error::CommandValidationError;
pub use command_rate_limit::PlayerCommandRateLimit;
pub use command_result::{CommandResult, CommandResultCode, CommandResultStatus};
pub use hrir_id::{HrirPresetId, HrirPresetIdError, MAX_HRIR_PRESET_ID_BYTES};
pub use id::{ChannelId, GuildId, SnowflakeParseError, UserId};
pub use player::{
    MAX_PLAYER_SNAPSHOT_JSON_BYTES, MAX_SNAPSHOT_QUEUE_ITEMS, MAX_TRACK_ALBUM_BYTES,
    MAX_TRACK_ARTIST_BYTES, MAX_TRACK_TITLE_BYTES, PlayerSnapshot, PlayerSnapshotValidationError,
    PlayerState, RepeatMode, TrackSnapshot,
};
pub use shard::shard_id;
pub use track_provenance::{MediaProvider, PublicMediaPage, PublicMediaPageError, TrackProvenance};
pub use volume::{Volume, VolumeError};
