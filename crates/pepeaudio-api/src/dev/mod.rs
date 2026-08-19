//! In-memory development and test adapters.
//!
//! These components are not durable or suitable for multi-replica deployments.

mod authorizer;
mod backend;
mod clock;
mod command_admission;
mod hrir_catalog;
mod player_commands;

pub use authorizer::AllowListAuthorizer;
pub use backend::InMemoryPlayerBackend;
pub use clock::ManualClock;
pub use hrir_catalog::StaticHrirPresetCatalog;
