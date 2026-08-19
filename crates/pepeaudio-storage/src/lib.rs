//! Persistence boundaries for `PepeAudio`.
//!
//! `PostgreSQL` contains durable product data. Valkey contains explicitly
//! disposable runtime snapshots, a recoverable shard command stream, and
//! bounded idempotency leases. Neither backend owns live Songbird state.

mod error;
mod model;
mod postgres;
mod repository;
mod valkey;

pub use error::{StorageError, StorageResult};
pub use model::{
    ControlPolicy, GuildSettings, HrirChannelLayout, HrirPresetMetadata, Playlist, PlaylistTrack,
    PlaylistVisibility, SettingsRevision, TrackSourceKind,
};
pub use postgres::PostgresStorage;
pub use repository::{GuildSettingsRepository, HrirPresetRepository, PlaylistRepository};
pub use valkey::{
    BotPresenceStore, ClaimBatch, CommandCompletion, CommandConsumer, CommandEnqueue,
    CommandProducer, CommandResultStore, CommandResultWrite, DedupeClaim, IdempotencyStore,
    Keyspace, ReceivedCommand, SnapshotEvent, SnapshotEventStream, SnapshotEventSubscriber,
    SnapshotStore, SnapshotWrite, ValkeyStore,
};
