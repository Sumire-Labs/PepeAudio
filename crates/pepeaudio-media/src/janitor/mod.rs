//! Safe bounded lifecycle management for downloaded media files.

mod model;
mod paths;
mod planning;
mod runtime;
pub(crate) mod scan;

pub use model::{
    DEFAULT_MAX_ENTRIES_PER_SCAN, DEFAULT_MAX_TOTAL_BYTES, DEFAULT_MINIMUM_OBJECT_RETENTION,
    DEFAULT_OBJECT_TTL, DEFAULT_STAGING_TTL, JanitorClock, JanitorError, JanitorPolicy,
    JanitorRemoval, JanitorRemovalReason, JanitorReport, JanitorSkip, JanitorSkipReason,
    SystemJanitorClock,
};
pub use runtime::ManagedDownloadJanitor;

pub(crate) use paths::ManagedPaths;
