//! In-process lifetime leases for completed managed media objects.

use std::{
    collections::HashMap,
    ffi::OsStr,
    fmt,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, Weak},
};

use tokio::fs;

use crate::{
    JanitorError, StoreError,
    capacity::{CapacityReservation, CapacityTracker, ManagedMediaCapacityUsage},
    janitor::{
        DEFAULT_MAX_ENTRIES_PER_SCAN, DEFAULT_MAX_TOTAL_BYTES, ManagedPaths,
        scan::is_link_or_reparse,
    },
};

/// Coordinates playback references with managed-object deletion.
///
/// Clone this registry into both the media resolver and janitor. Acquiring a
/// lease and reserving a path for deletion are serialized in memory, closing
/// the otherwise unavoidable check-then-delete race.
#[derive(Clone)]
pub struct ManagedMediaLeaseRegistry {
    inner: Arc<RegistryInner>,
    capacity: CapacityTracker,
    canonical_root: PathBuf,
    objects: PathBuf,
}

impl ManagedMediaLeaseRegistry {
    /// Prepares and validates a managed root before accepting leases.
    ///
    /// # Errors
    ///
    /// Returns [`JanitorError`] when the root or either managed child cannot
    /// be proven safe.
    pub async fn new(root: impl AsRef<Path>) -> Result<Self, JanitorError> {
        Self::new_with_capacity(root, DEFAULT_MAX_TOTAL_BYTES, DEFAULT_MAX_ENTRIES_PER_SCAN).await
    }

    /// Initializes exact byte and entry accounting for a private managed root.
    ///
    /// Both managed directories are inspected without following links. Any
    /// unknown entry, inspection ambiguity, arithmetic overflow, or scan-limit
    /// overflow fails startup closed.
    ///
    /// # Errors
    ///
    /// Returns [`JanitorError`] when root validation or exact startup
    /// accounting cannot be completed safely within the supplied limits.
    pub async fn new_with_capacity(
        root: impl AsRef<Path>,
        maximum_bytes: u64,
        maximum_entries: usize,
    ) -> Result<Self, JanitorError> {
        let paths = ManagedPaths::prepare(root.as_ref()).await?;
        let capacity = CapacityTracker::initialize(&paths, maximum_bytes, maximum_entries).await?;
        Ok(Self::from_paths_and_capacity(&paths, capacity))
    }

    pub(crate) fn from_paths(paths: &ManagedPaths) -> Self {
        Self::from_paths_and_capacity(paths, CapacityTracker::unmetered())
    }

    fn from_paths_and_capacity(paths: &ManagedPaths, capacity: CapacityTracker) -> Self {
        Self {
            inner: Arc::new(RegistryInner::default()),
            capacity,
            canonical_root: paths.root.clone(),
            objects: paths.objects.clone(),
        }
    }

    /// Acquires a shared lifetime lease for one completed managed object.
    ///
    /// This performs filesystem validation only at the ingestion boundary;
    /// cloning and dropping the resulting lease perform no filesystem I/O.
    ///
    /// # Errors
    ///
    /// Returns [`ManagedMediaLeaseError`] for a non-canonical, non-regular, or
    /// out-of-root object, or when cleanup already owns its deletion permit.
    pub async fn acquire(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<ManagedMediaLease, ManagedMediaLeaseError> {
        let path = path.as_ref();
        let metadata = fs::symlink_metadata(path)
            .await
            .map_err(ManagedMediaLeaseError::Inspect)?;
        if !metadata.is_file() || is_link_or_reparse(&metadata) {
            return Err(ManagedMediaLeaseError::UnsafeObject);
        }
        let canonical = fs::canonicalize(path)
            .await
            .map_err(ManagedMediaLeaseError::Inspect)?;
        if canonical.parent() != Some(self.objects.as_path())
            || !is_managed_object_name(canonical.file_name())
        {
            return Err(ManagedMediaLeaseError::UnsafeObject);
        }

        let mut entries = self.inner.entries();
        match entries.get_mut(&canonical) {
            Some(RegistryState::Leased(count)) => {
                *count = count
                    .checked_add(1)
                    .ok_or(ManagedMediaLeaseError::LeaseCountExhausted)?;
            }
            Some(RegistryState::Deleting) => {
                return Err(ManagedMediaLeaseError::DeletionInProgress);
            }
            None => {
                entries.insert(canonical.clone(), RegistryState::Leased(1));
            }
        }
        drop(entries);

        Ok(ManagedMediaLease {
            token: Arc::new(LeaseToken {
                registry: Arc::downgrade(&self.inner),
                path: canonical,
            }),
        })
    }

    #[must_use]
    pub fn canonical_root(&self) -> &Path {
        &self.canonical_root
    }

    /// Configured hard byte budget, or `None` for an internal janitor-only registry.
    #[must_use]
    pub fn maximum_bytes(&self) -> Option<u64> {
        self.capacity.maximum_bytes()
    }

    /// Bytes currently charged to safe files and in-flight reservations.
    #[must_use]
    pub fn charged_bytes(&self) -> Option<u64> {
        self.capacity.charged_bytes()
    }

    /// Returns path-free byte and entry counters for internal telemetry.
    #[must_use]
    pub fn capacity_usage(&self) -> Option<ManagedMediaCapacityUsage> {
        self.capacity.usage()
    }

    pub(crate) fn reserve(&self, requested_bytes: u64) -> Result<CapacityReservation, StoreError> {
        self.capacity.reserve(requested_bytes)
    }

    pub(crate) fn protects(&self, path: &Path) -> bool {
        self.inner.entries().contains_key(path)
    }

    pub(crate) fn begin_deletion(&self, path: &Path) -> Option<ManagedDeletionPermit> {
        let mut entries = self.inner.entries();
        if entries.contains_key(path) {
            return None;
        }
        entries.insert(path.to_path_buf(), RegistryState::Deleting);
        Some(ManagedDeletionPermit {
            registry: Arc::downgrade(&self.inner),
            capacity: self.capacity.clone(),
            path: path.to_path_buf(),
        })
    }
}

impl fmt::Debug for ManagedMediaLeaseRegistry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ManagedMediaLeaseRegistry")
            .field("canonical_root", &self.canonical_root)
            .field("maximum_bytes", &self.maximum_bytes())
            .finish_non_exhaustive()
    }
}

/// Arc-backed guard retained opaquely by a playback source.
#[derive(Clone)]
pub struct ManagedMediaLease {
    token: Arc<LeaseToken>,
}

impl ManagedMediaLease {
    #[must_use]
    pub fn canonical_path(&self) -> &Path {
        &self.token.path
    }
}

impl fmt::Debug for ManagedMediaLease {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ManagedMediaLease(<opaque>)")
    }
}

/// Failure to safely acquire an object lease.
#[derive(Debug, thiserror::Error)]
pub enum ManagedMediaLeaseError {
    #[error("could not inspect the managed media object")]
    Inspect(#[source] std::io::Error),
    #[error("media object is outside the safe managed object directory")]
    UnsafeObject,
    #[error("media object deletion is already in progress")]
    DeletionInProgress,
    #[error("managed media object has too many independent leases")]
    LeaseCountExhausted,
}

#[derive(Default)]
struct RegistryInner {
    entries: Mutex<HashMap<PathBuf, RegistryState>>,
}

impl RegistryInner {
    fn entries(&self) -> std::sync::MutexGuard<'_, HashMap<PathBuf, RegistryState>> {
        self.entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

enum RegistryState {
    Leased(usize),
    Deleting,
}

struct LeaseToken {
    registry: Weak<RegistryInner>,
    path: PathBuf,
}

impl Drop for LeaseToken {
    fn drop(&mut self) {
        let Some(registry) = self.registry.upgrade() else {
            return;
        };
        let mut entries = registry.entries();
        match entries.get_mut(&self.path) {
            Some(RegistryState::Leased(1)) => {
                entries.remove(&self.path);
            }
            Some(RegistryState::Leased(count)) => {
                *count -= 1;
            }
            Some(RegistryState::Deleting) | None => {}
        }
    }
}

pub(crate) struct ManagedDeletionPermit {
    registry: Weak<RegistryInner>,
    capacity: CapacityTracker,
    path: PathBuf,
}

impl ManagedDeletionPermit {
    pub(crate) fn removed(self) {
        self.capacity.removed(&self.path);
    }
}

impl Drop for ManagedDeletionPermit {
    fn drop(&mut self) {
        let Some(registry) = self.registry.upgrade() else {
            return;
        };
        let mut entries = registry.entries();
        if matches!(entries.get(&self.path), Some(RegistryState::Deleting)) {
            entries.remove(&self.path);
        }
    }
}

fn is_managed_object_name(name: Option<&OsStr>) -> bool {
    name.and_then(OsStr::to_str).is_some_and(|identifier| {
        identifier.len() == 32
            && identifier
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}
