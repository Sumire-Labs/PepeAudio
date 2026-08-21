//! Production-facing adapters shared by API and Discord shard processes.
//!
//! This crate connects transport-neutral application ports to Valkey without
//! making Valkey or Pub/Sub authoritative for live Songbird state.

#![forbid(unsafe_code)]

mod api_backend;
mod api_backend_runtime;
mod command_authorization;
mod command_dispatch;
mod command_loop;
mod command_outcome;
mod command_worker;
mod error;
mod event_hub;
mod guild_command_dispatcher;
mod guild_presence;
mod settings_model;
mod settings_persistence;
mod settings_publisher;
mod settings_worker;
mod snapshot_publisher;
mod snapshot_worker;

#[cfg(test)]
mod command_dispatch_tests;
#[cfg(test)]
mod command_loop_tests;
#[cfg(test)]
mod settings_persistence_tests;
#[cfg(test)]
mod snapshot_publisher_tests;

pub use api_backend::ValkeyApiBackend;
pub use api_backend_runtime::ApiBackendRuntime;
pub use command_authorization::{CommandAuthorization, CommandAuthorizer};
pub use command_worker::{
    CommandExecutionError, CommandWorkerConfig, CommandWorkerRuntime,
    DEFAULT_COMMAND_RESULT_RETENTION, PlayerDirectory, WorkerPlayerError,
};
pub use error::{RuntimeError, RuntimeResult};
pub use guild_presence::{GuildPresenceHandle, GuildPresenceRuntime};
pub use settings_model::{PersistentPlayerSettings, SettingsPersistenceView};
pub use settings_persistence::{
    SettingsPersistenceHandle, SettingsPersistenceRuntime, SettingsSupervisorError,
};
pub use settings_publisher::{
    PersistentSnapshotPublishError, PersistentSnapshotPublisher, SettingsPublishError,
    SettingsSnapshotPublisher,
};
pub use snapshot_publisher::{
    SnapshotPublishError, SnapshotPublisherHandle, SnapshotPublisherRuntime,
    SnapshotSupervisorError, ValkeySnapshotPublisher,
};
