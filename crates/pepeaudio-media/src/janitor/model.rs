use std::{path::PathBuf, time::Duration, time::SystemTime};

pub const DEFAULT_STAGING_TTL: Duration = Duration::from_hours(1);
pub const DEFAULT_OBJECT_TTL: Duration = Duration::from_hours(7 * 24);
pub const DEFAULT_MINIMUM_OBJECT_RETENTION: Duration = Duration::from_mins(5);
pub const DEFAULT_MAX_TOTAL_BYTES: u64 = 10 * 1024 * 1024 * 1024;
pub const DEFAULT_MAX_ENTRIES_PER_SCAN: usize = 4_096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JanitorPolicy {
    pub staging_ttl: Duration,
    pub object_ttl: Duration,
    /// Young objects protected from capacity-driven eviction.
    pub minimum_object_retention: Duration,
    /// Maximum bytes retained among managed files observed by a scan.
    pub max_total_bytes: u64,
    pub max_entries_per_scan: usize,
    /// Report intended removals without modifying the filesystem.
    pub dry_run: bool,
}

impl Default for JanitorPolicy {
    fn default() -> Self {
        Self {
            staging_ttl: DEFAULT_STAGING_TTL,
            object_ttl: DEFAULT_OBJECT_TTL,
            minimum_object_retention: DEFAULT_MINIMUM_OBJECT_RETENTION,
            max_total_bytes: DEFAULT_MAX_TOTAL_BYTES,
            max_entries_per_scan: DEFAULT_MAX_ENTRIES_PER_SCAN,
            dry_run: false,
        }
    }
}

impl JanitorPolicy {
    pub(crate) fn validate(self) -> Result<Self, JanitorError> {
        if self.staging_ttl.is_zero()
            || self.object_ttl.is_zero()
            || self.minimum_object_retention.is_zero()
            || self.minimum_object_retention > self.object_ttl
            || self.max_total_bytes == 0
            || self.max_entries_per_scan == 0
        {
            return Err(JanitorError::InvalidPolicy);
        }
        Ok(self)
    }
}

/// Injectable wall clock used to make retention decisions deterministic.
pub trait JanitorClock: Send + Sync {
    /// Returns the wall-clock instant used for an entire cleanup run.
    fn now(&self) -> SystemTime;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SystemJanitorClock;

impl JanitorClock for SystemJanitorClock {
    fn now(&self) -> SystemTime {
        SystemTime::now()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JanitorRemovalReason {
    StagingExpired,
    ObjectExpired,
    Capacity,
}

/// One successful removal, or one proposed removal in dry-run mode.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JanitorRemoval {
    /// Exact canonical direct child selected for removal.
    pub path: PathBuf,
    /// File size observed immediately before planning.
    pub size_bytes: u64,
    pub reason: JanitorRemovalReason,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JanitorSkipReason {
    ActivelyLeased,
    UnmanagedName,
    LinkOrReparsePoint,
    NonRegularFile,
    InspectionFailed,
    OutsideManagedDirectory,
    ChangedDuringScan,
    RemovalFailed,
}

/// One bounded diagnostic for an entry that was not modified.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JanitorSkip {
    /// Entry path as encountered below a canonical managed directory.
    pub path: PathBuf,
    pub reason: JanitorSkipReason,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JanitorReport {
    /// Canonical managed root used as the deletion trust anchor.
    pub canonical_root: PathBuf,
    pub dry_run: bool,
    pub scanned_entries: usize,
    /// True when more directory entries may exist beyond this run's bound.
    pub scan_limit_reached: bool,
    /// Regular, canonical, managed-name files included in capacity accounting.
    pub observed_files: usize,
    /// Bytes in safe managed files observed by this bounded scan.
    pub observed_bytes: u64,
    /// Observed bytes remaining after successful or proposed removals.
    pub retained_observed_bytes: u64,
    pub removals: Vec<JanitorRemoval>,
    /// Entries retained because they could not be proven safe to remove.
    pub skipped: Vec<JanitorSkip>,
}

impl JanitorReport {
    /// Observed bytes still above the configured budget after this run.
    #[must_use]
    pub fn observed_excess_bytes(&self, policy: JanitorPolicy) -> u64 {
        self.retained_observed_bytes
            .saturating_sub(policy.max_total_bytes)
    }
}

/// Failure that prevents a safe managed-root scan from running.
#[derive(Debug, thiserror::Error)]
pub enum JanitorError {
    #[error("managed download janitor policy is invalid")]
    InvalidPolicy,
    #[error("managed media hard-capacity policy is invalid")]
    InvalidCapacity,
    #[error("managed media startup accounting exceeded its entry limit")]
    CapacityScanLimitExceeded,
    #[error("managed media startup accounting found an unsafe entry")]
    UnsafeManagedEntry,
    #[error("managed media startup byte accounting overflowed")]
    CapacityAccountingOverflow,
    #[error("managed media startup usage exceeds its hard byte limit")]
    CapacityExceededAtStartup,
    #[error("could not prepare the managed download janitor root")]
    Prepare(#[source] std::io::Error),
    #[error("managed download root is not a safe canonical directory")]
    UnsafeRoot,
    #[error("managed download child directory is unsafe: {0}")]
    UnsafeManagedDirectory(&'static str),
    #[error("managed media lease registry belongs to a different root")]
    LeaseRegistryRootMismatch,
    #[error("could not enumerate a managed download directory")]
    ReadDirectory(#[source] std::io::Error),
    #[error("managed download janitor interval must be non-zero")]
    InvalidInterval,
}
