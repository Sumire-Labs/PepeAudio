mod support;

use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, SystemTime},
};

use pepeaudio_media::{
    JanitorClock, JanitorError, JanitorPolicy, JanitorRemovalReason, JanitorSkipReason,
    ManagedDownloadJanitor, ManagedMediaLeaseRegistry,
};
use support::TestRoot;

const OBJECT_A: &str = "0000000000000000000000000000000a";
const OBJECT_B: &str = "0000000000000000000000000000000b";
const OBJECT_C: &str = "0000000000000000000000000000000c";
const PARTIAL: &str = "0000000000000000000000000000000d.part";

#[derive(Clone, Copy, Debug)]
struct FixedClock(SystemTime);

impl JanitorClock for FixedClock {
    fn now(&self) -> SystemTime {
        self.0
    }
}

fn policy() -> JanitorPolicy {
    JanitorPolicy {
        staging_ttl: Duration::from_hours(1),
        object_ttl: Duration::from_hours(7 * 24),
        minimum_object_retention: Duration::from_hours(24),
        max_total_bytes: 1024,
        max_entries_per_scan: 64,
        dry_run: false,
    }
}

#[test]
fn defaults_keep_a_short_unleased_capacity_eviction_floor() {
    let defaults = JanitorPolicy::default();

    assert_eq!(defaults.minimum_object_retention, Duration::from_mins(5));
    assert!(defaults.object_ttl >= defaults.minimum_object_retention);
    assert!(defaults.staging_ttl > Duration::ZERO);
    assert!(defaults.max_total_bytes > 0);
    assert!(defaults.max_entries_per_scan > 0);
    assert!(!defaults.dry_run);
}

async fn managed_file(root: &Path, directory: &str, name: &str, bytes: &[u8]) -> PathBuf {
    let directory = root.join(directory);
    tokio::fs::create_dir_all(&directory)
        .await
        .expect("managed directory");
    let path = directory.join(name);
    tokio::fs::write(&path, bytes).await.expect("managed file");
    path
}

async fn modified(path: &Path) -> SystemTime {
    tokio::fs::metadata(path)
        .await
        .expect("metadata")
        .modified()
        .expect("modified time")
}

async fn fixed_janitor(
    root: &Path,
    policy: JanitorPolicy,
    now: SystemTime,
) -> ManagedDownloadJanitor<FixedClock> {
    ManagedDownloadJanitor::with_clock(root, policy, FixedClock(now))
        .await
        .expect("janitor")
}

async fn leased_fixed_janitor(
    root: &Path,
    policy: JanitorPolicy,
    now: SystemTime,
    leases: ManagedMediaLeaseRegistry,
) -> ManagedDownloadJanitor<FixedClock> {
    ManagedDownloadJanitor::with_clock_and_registry(root, policy, FixedClock(now), leases)
        .await
        .expect("leased janitor")
}

#[tokio::test]
async fn expired_staging_and_objects_are_removed() {
    let root = TestRoot::new("janitor-expired");
    let partial = managed_file(&root.0, "staging", PARTIAL, b"partial").await;
    let object = managed_file(&root.0, "objects", OBJECT_A, b"object").await;
    let mut cleanup = policy();
    cleanup.object_ttl = Duration::from_hours(1);
    cleanup.minimum_object_retention = Duration::from_mins(30);
    let now = modified(&partial).await + Duration::from_hours(2);
    let janitor = fixed_janitor(&root.0, cleanup, now).await;

    let report = janitor.run().await.expect("cleanup report");

    assert!(!partial.exists());
    assert!(!object.exists());
    assert_eq!(report.observed_files, 2);
    assert_eq!(report.retained_observed_bytes, 0);
    assert!(report.removals.iter().any(|removal| {
        removal.reason == JanitorRemovalReason::StagingExpired && removal.path.ends_with(PARTIAL)
    }));
    assert!(report.removals.iter().any(|removal| {
        removal.reason == JanitorRemovalReason::ObjectExpired && removal.path.ends_with(OBJECT_A)
    }));
}

#[tokio::test]
async fn capacity_never_evicts_an_object_inside_minimum_retention() {
    let root = TestRoot::new("janitor-young");
    let object = managed_file(&root.0, "objects", OBJECT_A, b"young object").await;
    let mut cleanup = policy();
    cleanup.max_total_bytes = 1;
    let now = modified(&object).await + Duration::from_hours(23);
    let janitor = fixed_janitor(&root.0, cleanup, now).await;

    let report = janitor.run().await.expect("cleanup report");

    assert!(object.exists());
    assert!(report.removals.is_empty());
    assert_eq!(report.observed_excess_bytes(cleanup), 11);
}

#[tokio::test]
async fn capacity_evicts_oldest_eligible_objects_only_as_needed() {
    let root = TestRoot::new("janitor-capacity");
    let first = managed_file(&root.0, "objects", OBJECT_A, b"aaaa").await;
    let second = managed_file(&root.0, "objects", OBJECT_B, b"bbbb").await;
    let mut cleanup = policy();
    cleanup.max_total_bytes = 4;
    let now = modified(&first).await + Duration::from_hours(25);
    let janitor = fixed_janitor(&root.0, cleanup, now).await;

    let report = janitor.run().await.expect("cleanup report");

    assert_eq!(report.removals.len(), 1);
    assert_eq!(report.removals[0].reason, JanitorRemovalReason::Capacity);
    assert_eq!(report.retained_observed_bytes, 4);
    assert_ne!(first.exists(), second.exists());
}

#[tokio::test]
async fn expired_object_survives_until_the_final_lease_clone_drops() {
    let root = TestRoot::new("janitor-leased-expired");
    let object = managed_file(&root.0, "objects", OBJECT_A, b"leased").await;
    let leases = ManagedMediaLeaseRegistry::new(&root.0)
        .await
        .expect("lease registry");
    let lease = leases.acquire(&object).await.expect("object lease");
    let lease_clone = lease.clone();
    let mut cleanup = policy();
    cleanup.object_ttl = Duration::from_hours(1);
    cleanup.minimum_object_retention = Duration::from_mins(30);
    let now = modified(&object).await + Duration::from_hours(2);
    let janitor = leased_fixed_janitor(&root.0, cleanup, now, leases).await;

    let first = janitor.run().await.expect("leased cleanup report");
    assert!(object.exists());
    assert!(first.removals.is_empty());
    assert!(
        first
            .skipped
            .iter()
            .any(|skip| skip.reason == JanitorSkipReason::ActivelyLeased)
    );

    drop(lease);
    janitor.run().await.expect("clone still protects object");
    assert!(object.exists());

    drop(lease_clone);
    let final_report = janitor.run().await.expect("unleased cleanup report");
    assert!(!object.exists());
    assert_eq!(final_report.removals.len(), 1);
    assert_eq!(
        final_report.removals[0].reason,
        JanitorRemovalReason::ObjectExpired
    );
}

#[tokio::test]
async fn capacity_pressure_cannot_evict_a_leased_object() {
    let root = TestRoot::new("janitor-leased-capacity");
    let object = managed_file(&root.0, "objects", OBJECT_A, b"leased capacity").await;
    let leases = ManagedMediaLeaseRegistry::new(&root.0)
        .await
        .expect("lease registry");
    let lease = leases.acquire(&object).await.expect("object lease");
    let mut cleanup = policy();
    cleanup.max_total_bytes = 1;
    let now = modified(&object).await + Duration::from_hours(25);
    let janitor = leased_fixed_janitor(&root.0, cleanup, now, leases).await;

    let leased_report = janitor.run().await.expect("leased capacity report");
    assert!(object.exists());
    assert!(leased_report.removals.is_empty());
    assert!(leased_report.observed_excess_bytes(cleanup) > 0);

    drop(lease);
    let unleased_report = janitor.run().await.expect("unleased capacity report");
    assert!(!object.exists());
    assert_eq!(unleased_report.removals.len(), 1);
    assert_eq!(
        unleased_report.removals[0].reason,
        JanitorRemovalReason::Capacity
    );
}

#[tokio::test]
async fn dry_run_reports_expiration_without_removing_the_file() {
    let root = TestRoot::new("janitor-dry-run");
    let object = managed_file(&root.0, "objects", OBJECT_A, b"expired").await;
    let mut cleanup = policy();
    cleanup.dry_run = true;
    let now = modified(&object).await + Duration::from_hours(8 * 24);
    let janitor = fixed_janitor(&root.0, cleanup, now).await;

    let report = janitor.run().await.expect("dry-run report");

    assert!(object.exists());
    assert!(report.dry_run);
    assert_eq!(report.removals.len(), 1);
    assert_eq!(report.retained_observed_bytes, 0);
}

#[tokio::test]
async fn scan_limit_bounds_work_and_reports_truncation() {
    let root = TestRoot::new("janitor-limit");
    let first = managed_file(&root.0, "objects", OBJECT_A, b"a").await;
    managed_file(&root.0, "objects", OBJECT_B, b"b").await;
    managed_file(&root.0, "objects", OBJECT_C, b"c").await;
    let mut cleanup = policy();
    cleanup.max_entries_per_scan = 2;
    cleanup.object_ttl = Duration::from_hours(1);
    cleanup.minimum_object_retention = Duration::from_mins(30);
    let now = modified(&first).await + Duration::from_hours(2);
    let janitor = fixed_janitor(&root.0, cleanup, now).await;

    let report = janitor.run().await.expect("bounded report");

    assert_eq!(report.scanned_entries, 2);
    assert!(report.scan_limit_reached);
    assert_eq!(report.removals.len(), 2);
    let remaining = std::fs::read_dir(root.0.join("objects"))
        .expect("objects")
        .count();
    assert_eq!(remaining, 1);
}

#[tokio::test]
async fn unmanaged_nonregular_and_link_entries_are_never_removed() {
    let root = TestRoot::new("janitor-unsafe");
    let outside = TestRoot::new("janitor-outside");
    let unknown = managed_file(&root.0, "objects", "operator-note", b"keep").await;
    let directory = root.0.join("objects").join(OBJECT_A);
    tokio::fs::create_dir(&directory)
        .await
        .expect("non-regular entry");
    tokio::fs::create_dir_all(&outside.0)
        .await
        .expect("outside root");
    let outside_file = outside.0.join("outside");
    tokio::fs::write(&outside_file, b"outside")
        .await
        .expect("outside file");
    let link = root.0.join("objects").join(OBJECT_B);
    let link_created = create_file_link(&outside_file, &link).is_ok();
    let now = SystemTime::now() + Duration::from_hours(30 * 24);
    let janitor = fixed_janitor(&root.0, policy(), now).await;

    let report = janitor.run().await.expect("safe report");

    assert!(unknown.exists());
    assert!(directory.is_dir());
    assert!(outside_file.exists());
    assert!(
        report
            .skipped
            .iter()
            .any(|skip| skip.reason == JanitorSkipReason::UnmanagedName)
    );
    assert!(
        report
            .skipped
            .iter()
            .any(|skip| skip.reason == JanitorSkipReason::NonRegularFile)
    );
    if link_created {
        assert!(link.symlink_metadata().is_ok());
        assert!(
            report
                .skipped
                .iter()
                .any(|skip| skip.reason == JanitorSkipReason::LinkOrReparsePoint)
        );
    }
}

#[tokio::test]
async fn replacing_managed_directory_with_a_link_aborts_before_scanning() {
    let root = TestRoot::new("janitor-linked-directory");
    let outside = TestRoot::new("janitor-linked-target");
    let now = SystemTime::now() + Duration::from_hours(30 * 24);
    let janitor = fixed_janitor(&root.0, policy(), now).await;
    tokio::fs::create_dir_all(&outside.0)
        .await
        .expect("outside root");
    let outside_file = managed_file(&outside.0, "nested", OBJECT_A, b"outside").await;
    let objects = root.0.join("objects");
    tokio::fs::remove_dir(&objects)
        .await
        .expect("empty objects directory");
    if create_directory_link(&outside.0, &objects).is_err() {
        return;
    }

    let error = janitor.run().await.expect_err("linked directory rejected");

    assert!(matches!(
        error,
        JanitorError::UnsafeManagedDirectory("objects")
    ));
    assert!(outside_file.exists());
}

#[tokio::test]
async fn invalid_policy_and_zero_periodic_interval_are_rejected() {
    let root = TestRoot::new("janitor-invalid");
    let mut invalid = policy();
    invalid.max_entries_per_scan = 0;
    assert!(matches!(
        ManagedDownloadJanitor::new(&root.0, invalid).await,
        Err(JanitorError::InvalidPolicy)
    ));

    let janitor = fixed_janitor(&root.0, policy(), SystemTime::now()).await;
    let (_shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let reports = AtomicUsize::new(0);
    let result = janitor
        .run_periodic(Duration::ZERO, shutdown_rx, |_| {
            reports.fetch_add(1, Ordering::Relaxed);
        })
        .await;
    assert!(matches!(result, Err(JanitorError::InvalidInterval)));
    assert_eq!(reports.load(Ordering::Relaxed), 0);
}

#[cfg(unix)]
fn create_file_link(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn create_file_link(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_file(target, link)
}

#[cfg(unix)]
fn create_directory_link(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn create_directory_link(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_dir(target, link)
}
