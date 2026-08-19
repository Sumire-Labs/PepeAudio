//! Fail-closed process-local accounting for one private managed-media root.

use std::{
    collections::HashMap,
    ffi::OsStr,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use tokio::fs;

use crate::{JanitorError, StoreError, janitor::ManagedPaths, janitor::scan::is_link_or_reparse};

/// Path-free process-local capacity counters suitable for logs and metrics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ManagedMediaCapacityUsage {
    pub used_bytes: u64,
    pub reserved_bytes: u64,
    pub maximum_bytes: u64,
    pub managed_files: usize,
    pub reservations: usize,
    pub maximum_entries: usize,
}

#[derive(Clone, Default)]
pub(crate) struct CapacityTracker {
    inner: Option<Arc<CapacityInner>>,
}

impl CapacityTracker {
    pub(crate) const fn unmetered() -> Self {
        Self { inner: None }
    }

    pub(crate) async fn initialize(
        paths: &ManagedPaths,
        maximum_bytes: u64,
        maximum_entries: usize,
    ) -> Result<Self, JanitorError> {
        if maximum_bytes == 0 || maximum_entries == 0 {
            return Err(JanitorError::InvalidCapacity);
        }
        paths.validate().await?;
        let mut entries = HashMap::new();
        let mut charged_bytes = 0_u64;
        scan_directory(
            &paths.staging,
            true,
            maximum_entries,
            &mut entries,
            &mut charged_bytes,
        )
        .await?;
        scan_directory(
            &paths.objects,
            false,
            maximum_entries,
            &mut entries,
            &mut charged_bytes,
        )
        .await?;
        if entries.len() > maximum_entries {
            return Err(JanitorError::CapacityScanLimitExceeded);
        }
        if charged_bytes > maximum_bytes {
            return Err(JanitorError::CapacityExceededAtStartup);
        }
        Ok(Self {
            inner: Some(Arc::new(CapacityInner {
                maximum_bytes,
                maximum_entries,
                state: Mutex::new(CapacityState {
                    charged_bytes,
                    entries,
                    reservations: 0,
                }),
            })),
        })
    }

    pub(crate) fn maximum_bytes(&self) -> Option<u64> {
        self.inner.as_ref().map(|inner| inner.maximum_bytes)
    }

    pub(crate) fn charged_bytes(&self) -> Option<u64> {
        self.inner.as_ref().map(|inner| inner.state().charged_bytes)
    }

    pub(crate) fn usage(&self) -> Option<ManagedMediaCapacityUsage> {
        self.inner.as_ref().map(|inner| {
            let state = inner.state();
            let used_bytes = state.entries.values().copied().sum();
            ManagedMediaCapacityUsage {
                used_bytes,
                reserved_bytes: state.charged_bytes.saturating_sub(used_bytes),
                maximum_bytes: inner.maximum_bytes,
                managed_files: state.entries.len(),
                reservations: state.reservations,
                maximum_entries: inner.maximum_entries,
            }
        })
    }

    pub(crate) fn reserve(&self, requested_bytes: u64) -> Result<CapacityReservation, StoreError> {
        let inner = self.inner.as_ref().ok_or(StoreError::CapacityUnavailable)?;
        let mut state = inner.state();
        if state.entries.len().saturating_add(state.reservations) >= inner.maximum_entries
            || !fits(state.charged_bytes, requested_bytes, inner.maximum_bytes)
        {
            return Err(StoreError::CapacityExceeded);
        }
        state.charged_bytes += requested_bytes;
        state.reservations += 1;
        Ok(CapacityReservation {
            inner: Arc::clone(inner),
            charged_bytes: requested_bytes,
            active: true,
        })
    }

    pub(crate) fn removed(&self, path: &Path) {
        let Some(inner) = &self.inner else { return };
        let mut state = inner.state();
        if let Some(bytes) = state.entries.remove(path) {
            state.charged_bytes = state.charged_bytes.saturating_sub(bytes);
        }
    }
}

pub(crate) struct CapacityReservation {
    inner: Arc<CapacityInner>,
    charged_bytes: u64,
    active: bool,
}

impl CapacityReservation {
    pub(crate) fn ensure(&mut self, requested_bytes: u64) -> Result<(), StoreError> {
        if requested_bytes <= self.charged_bytes {
            return Ok(());
        }
        let additional = requested_bytes - self.charged_bytes;
        let mut state = self.inner.state();
        if !fits(state.charged_bytes, additional, self.inner.maximum_bytes) {
            return Err(StoreError::CapacityExceeded);
        }
        state.charged_bytes += additional;
        self.charged_bytes = requested_bytes;
        Ok(())
    }

    pub(crate) fn commit(&mut self, path: PathBuf, actual_bytes: u64) -> Result<(), StoreError> {
        if !self.active || actual_bytes == 0 || actual_bytes > self.charged_bytes {
            return Err(StoreError::Accounting);
        }
        let mut state = self.inner.state();
        if state.entries.contains_key(&path) || state.reservations == 0 {
            return Err(StoreError::Accounting);
        }
        state.charged_bytes = state
            .charged_bytes
            .checked_sub(self.charged_bytes)
            .and_then(|value| value.checked_add(actual_bytes))
            .ok_or(StoreError::Accounting)?;
        state.reservations -= 1;
        state.entries.insert(path, actual_bytes);
        self.active = false;
        Ok(())
    }

    pub(crate) fn retain_failed_file(&mut self, path: PathBuf) {
        if !self.active {
            return;
        }
        let mut state = self.inner.state();
        if state.reservations > 0 {
            state.reservations -= 1;
        }
        if state.entries.contains_key(&path) {
            // Preserve the extra byte charge even after an impossible UUID
            // collision. It can be reset only by a safe process restart scan,
            // which is preferable to undercounting an uncertain file.
            self.active = false;
            return;
        }
        state.entries.insert(path, self.charged_bytes);
        self.active = false;
    }
}

impl Drop for CapacityReservation {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        let mut state = self.inner.state();
        state.charged_bytes = state.charged_bytes.saturating_sub(self.charged_bytes);
        state.reservations = state.reservations.saturating_sub(1);
    }
}

struct CapacityInner {
    maximum_bytes: u64,
    maximum_entries: usize,
    state: Mutex<CapacityState>,
}

impl CapacityInner {
    fn state(&self) -> std::sync::MutexGuard<'_, CapacityState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

struct CapacityState {
    charged_bytes: u64,
    entries: HashMap<PathBuf, u64>,
    reservations: usize,
}

fn fits(current: u64, additional: u64, maximum: u64) -> bool {
    current
        .checked_add(additional)
        .is_some_and(|total| total <= maximum)
}

async fn scan_directory(
    directory: &Path,
    staging: bool,
    maximum_entries: usize,
    entries: &mut HashMap<PathBuf, u64>,
    charged_bytes: &mut u64,
) -> Result<(), JanitorError> {
    let mut reader = fs::read_dir(directory)
        .await
        .map_err(JanitorError::ReadDirectory)?;
    while let Some(entry) = reader
        .next_entry()
        .await
        .map_err(JanitorError::ReadDirectory)?
    {
        if entries.len() >= maximum_entries {
            return Err(JanitorError::CapacityScanLimitExceeded);
        }
        let path = entry.path();
        if !is_managed_name(entry.file_name().as_os_str(), staging) {
            return Err(JanitorError::UnsafeManagedEntry);
        }
        let metadata = fs::symlink_metadata(&path)
            .await
            .map_err(|_| JanitorError::UnsafeManagedEntry)?;
        if !metadata.is_file() || is_link_or_reparse(&metadata) {
            return Err(JanitorError::UnsafeManagedEntry);
        }
        let canonical = fs::canonicalize(&path)
            .await
            .map_err(|_| JanitorError::UnsafeManagedEntry)?;
        if canonical != path || canonical.parent() != Some(directory) {
            return Err(JanitorError::UnsafeManagedEntry);
        }
        *charged_bytes = charged_bytes
            .checked_add(metadata.len())
            .ok_or(JanitorError::CapacityAccountingOverflow)?;
        if entries.insert(canonical, metadata.len()).is_some() {
            return Err(JanitorError::UnsafeManagedEntry);
        }
    }
    Ok(())
}

fn is_managed_name(name: &OsStr, staging: bool) -> bool {
    let Some(name) = name.to_str() else {
        return false;
    };
    let identifier = if staging {
        name.strip_suffix(".part")
    } else {
        Some(name)
    };
    identifier.is_some_and(|identifier| {
        identifier.len() == 32
            && identifier
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

#[cfg(test)]
mod tests;
