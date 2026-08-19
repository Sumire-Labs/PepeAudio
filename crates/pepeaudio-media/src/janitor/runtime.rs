use std::{path::Path, time::Duration};

use tokio::sync::{Mutex, watch};

use super::{
    JanitorClock, JanitorError, JanitorPolicy, JanitorReport, ManagedPaths, SystemJanitorClock,
    scan::run_scan,
};
use crate::lease::ManagedMediaLeaseRegistry;

/// Bounded janitor for one [`crate::DownloadStore`] compatible managed root.
///
/// Construct this once at startup, run it immediately, then call [`Self::run`]
/// periodically or use [`Self::run_periodic`]. The canonical root and its two
/// direct managed directories are revalidated before every scan.
#[derive(Debug)]
pub struct ManagedDownloadJanitor<C = SystemJanitorClock> {
    paths: ManagedPaths,
    policy: JanitorPolicy,
    clock: C,
    leases: ManagedMediaLeaseRegistry,
    scan_lock: Mutex<()>,
}

impl ManagedDownloadJanitor<SystemJanitorClock> {
    /// Uses the production clock and an internal lease registry.
    ///
    /// # Errors
    ///
    /// Returns [`JanitorError`] for an invalid policy or unsafe managed root.
    pub async fn new(root: impl AsRef<Path>, policy: JanitorPolicy) -> Result<Self, JanitorError> {
        Self::with_clock(root, policy, SystemJanitorClock).await
    }

    /// Uses the production clock and the ingestion lease registry.
    ///
    /// # Errors
    ///
    /// Returns [`JanitorError`] for an invalid policy, unsafe root, or registry
    /// belonging to another canonical root.
    pub async fn new_with_registry(
        root: impl AsRef<Path>,
        policy: JanitorPolicy,
        leases: ManagedMediaLeaseRegistry,
    ) -> Result<Self, JanitorError> {
        Self::with_clock_and_registry(root, policy, SystemJanitorClock, leases).await
    }
}

impl<C: JanitorClock> ManagedDownloadJanitor<C> {
    /// Uses an injected wall clock and an internal lease registry.
    ///
    /// # Errors
    ///
    /// Returns [`JanitorError`] for an invalid policy or unsafe managed root.
    pub async fn with_clock(
        root: impl AsRef<Path>,
        policy: JanitorPolicy,
        clock: C,
    ) -> Result<Self, JanitorError> {
        let policy = policy.validate()?;
        let paths = ManagedPaths::prepare(root.as_ref()).await?;
        let leases = ManagedMediaLeaseRegistry::from_paths(&paths);
        Ok(Self {
            paths,
            policy,
            clock,
            leases,
            scan_lock: Mutex::new(()),
        })
    }

    /// Uses an injected wall clock and caller-owned lease registry.
    ///
    /// # Errors
    ///
    /// Returns [`JanitorError`] for an invalid policy, unsafe root, or registry
    /// belonging to another canonical root.
    pub async fn with_clock_and_registry(
        root: impl AsRef<Path>,
        policy: JanitorPolicy,
        clock: C,
        leases: ManagedMediaLeaseRegistry,
    ) -> Result<Self, JanitorError> {
        let policy = policy.validate()?;
        let paths = ManagedPaths::prepare(root.as_ref()).await?;
        if leases.canonical_root() != paths.root {
            return Err(JanitorError::LeaseRegistryRootMismatch);
        }
        Ok(Self {
            paths,
            policy,
            clock,
            leases,
            scan_lock: Mutex::new(()),
        })
    }

    /// Canonical managed root used as the deletion trust anchor.
    #[must_use]
    pub fn canonical_root(&self) -> &Path {
        &self.paths.root
    }

    #[must_use]
    pub const fn policy(&self) -> JanitorPolicy {
        self.policy
    }

    /// Runs one bounded scan using a single injected wall-clock instant.
    ///
    /// # Errors
    ///
    /// Returns [`JanitorError`] if the canonical directory boundary changed or
    /// a managed directory cannot be enumerated. Individual unsafe entries and
    /// deletion failures are retained and included in the report instead.
    pub async fn run(&self) -> Result<JanitorReport, JanitorError> {
        let _scan_guard = self.scan_lock.lock().await;
        run_scan(&self.paths, &self.leases, self.policy, self.clock.now()).await
    }

    /// Runs a serialized cleanup pass targeting enough free space for one
    /// prospective reservation. Only unleased objects older than
    /// [`JanitorPolicy::minimum_object_retention`] are capacity candidates.
    /// A later reservation can still lose to concurrent ingest and must remain
    /// fail closed.
    ///
    /// # Errors
    ///
    /// Returns the same root-safety and enumeration failures as [`Self::run`].
    pub async fn run_for_admission(
        &self,
        requested_bytes: u64,
    ) -> Result<JanitorReport, JanitorError> {
        let _scan_guard = self.scan_lock.lock().await;
        let mut policy = self.policy;
        policy.max_total_bytes = policy.max_total_bytes.saturating_sub(requested_bytes);
        run_scan(&self.paths, &self.leases, policy, self.clock.now()).await
    }

    /// Runs once at startup and then at each interval until shutdown is true.
    ///
    /// The callback receives each report synchronously. Dropping the shutdown
    /// sender also stops the runner. A root-safety or enumeration failure stops
    /// the loop and is returned to the caller.
    ///
    /// # Errors
    ///
    /// Returns [`JanitorError::InvalidInterval`] for a zero interval, or the
    /// first error returned by [`Self::run`].
    pub async fn run_periodic<F>(
        &self,
        interval: Duration,
        mut shutdown: watch::Receiver<bool>,
        mut on_report: F,
    ) -> Result<(), JanitorError>
    where
        F: FnMut(JanitorReport),
    {
        if interval.is_zero() {
            return Err(JanitorError::InvalidInterval);
        }
        loop {
            if *shutdown.borrow() {
                return Ok(());
            }
            on_report(self.run().await?);
            if let Ok(Err(_)) = tokio::time::timeout(interval, shutdown.changed()).await {
                return Ok(());
            }
        }
    }
}
