//! Transport-independent, per-guild playback runtime.
//!
//! Discord, Songbird, and persistence adapters implement the ports exported by
//! this crate. The authoritative player itself is a bounded Tokio actor.

mod actor;
mod config;
mod error;
mod event;
mod handle;
mod model;
mod ports;
mod request;
mod timer;
mod track_metadata;

pub use config::{DEFAULT_IDLE_TIMEOUT, PlayerConfig, PlayerConfigError};
pub use error::{PlayerError, SideEffect};
pub use event::{
    IdleGeneration, PlaybackEndReason, PlaybackGeneration, PlaybackIdentity, PlayerEvent,
    ShutdownReport,
};
pub use handle::{PlayerHandle, PlayerRuntime, spawn_player, spawn_player_with_revision};
pub use model::{PlaybackSource, QueueTrack};
pub use ports::{NoopPlayback, NoopSnapshotPublisher, PlaybackPort, SnapshotPublisher};
pub use track_metadata::{
    MAX_TRACK_ALBUM_BYTES, MAX_TRACK_ARTIST_BYTES, QueueTrackMetadata, QueueTrackMetadataBuilder,
    TrackMetadataError, TrackMetadataField,
};
