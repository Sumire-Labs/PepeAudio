use std::{
    ffi::OsStr,
    fs::Metadata,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use tokio::fs;

use super::{
    JanitorError, JanitorPolicy, JanitorRemoval, JanitorRemovalReason, JanitorReport, JanitorSkip,
    JanitorSkipReason, ManagedPaths,
    planning::{Candidate, ManagedKind, Plan, old_enough, plan_removals},
};
use crate::lease::ManagedMediaLeaseRegistry;

struct DirectoryScan {
    kind: ManagedKind,
    directory: PathBuf,
    entries: fs::ReadDir,
    exhausted: bool,
}

pub(super) async fn run_scan(
    paths: &ManagedPaths,
    leases: &ManagedMediaLeaseRegistry,
    policy: JanitorPolicy,
    now: SystemTime,
) -> Result<JanitorReport, JanitorError> {
    paths.validate().await?;
    let mut report = JanitorReport {
        canonical_root: paths.root.clone(),
        dry_run: policy.dry_run,
        scanned_entries: 0,
        scan_limit_reached: false,
        observed_files: 0,
        observed_bytes: 0,
        retained_observed_bytes: 0,
        removals: Vec::new(),
        skipped: Vec::new(),
    };
    let (candidates, limit_reached) = scan_candidates(paths, leases, policy, &mut report).await?;
    report.scan_limit_reached = limit_reached;
    report.observed_files = candidates.len();
    report.observed_bytes = candidates
        .iter()
        .fold(0_u64, |sum, item| sum.saturating_add(item.size_bytes));

    for candidate in candidates.iter().filter(|candidate| candidate.leased) {
        skip(
            &mut report,
            candidate.path.clone(),
            JanitorSkipReason::ActivelyLeased,
        );
    }
    let plans = plan_removals(&candidates, policy, now, report.observed_bytes);
    apply_plans(paths, leases, &candidates, plans, policy, now, &mut report).await;
    report.retained_observed_bytes = report.observed_bytes.saturating_sub(
        report
            .removals
            .iter()
            .fold(0_u64, |sum, removal| sum.saturating_add(removal.size_bytes)),
    );
    Ok(report)
}

async fn scan_candidates(
    paths: &ManagedPaths,
    leases: &ManagedMediaLeaseRegistry,
    policy: JanitorPolicy,
    report: &mut JanitorReport,
) -> Result<(Vec<Candidate>, bool), JanitorError> {
    let mut directories = [
        DirectoryScan {
            kind: ManagedKind::Staging,
            directory: paths.staging.clone(),
            entries: fs::read_dir(&paths.staging)
                .await
                .map_err(JanitorError::ReadDirectory)?,
            exhausted: false,
        },
        DirectoryScan {
            kind: ManagedKind::Object,
            directory: paths.objects.clone(),
            entries: fs::read_dir(&paths.objects)
                .await
                .map_err(JanitorError::ReadDirectory)?,
            exhausted: false,
        },
    ];
    let mut candidates = Vec::new();
    let mut cursor = 0;

    while report.scanned_entries < policy.max_entries_per_scan
        && directories.iter().any(|directory| !directory.exhausted)
    {
        let index = next_directory(&directories, cursor);
        cursor = (index + 1) % directories.len();
        let directory = &mut directories[index];
        match directory
            .entries
            .next_entry()
            .await
            .map_err(JanitorError::ReadDirectory)?
        {
            Some(entry) => {
                report.scanned_entries += 1;
                inspect_entry(
                    directory.kind,
                    &directory.directory,
                    entry,
                    leases,
                    &mut candidates,
                    report,
                )
                .await;
            }
            None => directory.exhausted = true,
        }
    }

    Ok((
        candidates,
        report.scanned_entries == policy.max_entries_per_scan
            && directories.iter().any(|directory| !directory.exhausted),
    ))
}

fn next_directory(directories: &[DirectoryScan; 2], cursor: usize) -> usize {
    if directories[cursor].exhausted {
        (cursor + 1) % directories.len()
    } else {
        cursor
    }
}

async fn inspect_entry(
    kind: ManagedKind,
    directory: &Path,
    entry: fs::DirEntry,
    leases: &ManagedMediaLeaseRegistry,
    candidates: &mut Vec<Candidate>,
    report: &mut JanitorReport,
) {
    let path = entry.path();
    if !is_managed_name(kind, &entry.file_name()) {
        skip(report, path, JanitorSkipReason::UnmanagedName);
        return;
    }
    let Ok(metadata) = fs::symlink_metadata(&path).await else {
        skip(report, path, JanitorSkipReason::InspectionFailed);
        return;
    };
    if is_link_or_reparse(&metadata) {
        skip(report, path, JanitorSkipReason::LinkOrReparsePoint);
        return;
    }
    if !metadata.is_file() {
        skip(report, path, JanitorSkipReason::NonRegularFile);
        return;
    }
    let Ok(canonical) = fs::canonicalize(&path).await else {
        skip(report, path, JanitorSkipReason::InspectionFailed);
        return;
    };
    if canonical.parent() != Some(directory) || canonical != path {
        skip(report, path, JanitorSkipReason::OutsideManagedDirectory);
        return;
    }
    let Ok(modified) = metadata.modified() else {
        skip(report, path, JanitorSkipReason::InspectionFailed);
        return;
    };
    candidates.push(Candidate {
        leased: kind == ManagedKind::Object && leases.protects(&canonical),
        path: canonical,
        kind,
        size_bytes: metadata.len(),
        modified,
    });
}

async fn apply_plans(
    paths: &ManagedPaths,
    leases: &ManagedMediaLeaseRegistry,
    candidates: &[Candidate],
    plans: Vec<Plan>,
    policy: JanitorPolicy,
    now: SystemTime,
    report: &mut JanitorReport,
) {
    for plan in plans {
        let candidate = &candidates[plan.index];
        if policy.dry_run {
            if leases.protects(&candidate.path) {
                skip(
                    report,
                    candidate.path.clone(),
                    JanitorSkipReason::ActivelyLeased,
                );
            } else {
                report.removals.push(removal(candidate, plan.reason));
            }
            continue;
        }
        let directory = match candidate.kind {
            ManagedKind::Staging => &paths.staging,
            ManagedKind::Object => &paths.objects,
        };
        let minimum_age = match plan.reason {
            JanitorRemovalReason::StagingExpired => policy.staging_ttl,
            JanitorRemovalReason::ObjectExpired => policy.object_ttl,
            JanitorRemovalReason::Capacity => policy.minimum_object_retention,
        };
        match verify_unchanged(candidate, directory, now, minimum_age).await {
            Ok(()) => {
                let Some(permit) = leases.begin_deletion(&candidate.path) else {
                    skip(
                        report,
                        candidate.path.clone(),
                        JanitorSkipReason::ActivelyLeased,
                    );
                    continue;
                };
                match fs::remove_file(&candidate.path).await {
                    Ok(()) => {
                        permit.removed();
                        report.removals.push(removal(candidate, plan.reason));
                    }
                    Err(_) => skip(
                        report,
                        candidate.path.clone(),
                        JanitorSkipReason::RemovalFailed,
                    ),
                }
            }
            Err(reason) => skip(report, candidate.path.clone(), reason),
        }
    }
}

async fn verify_unchanged(
    candidate: &Candidate,
    directory: &Path,
    now: SystemTime,
    minimum_age: Duration,
) -> Result<(), JanitorSkipReason> {
    if candidate.path.parent() != Some(directory) {
        return Err(JanitorSkipReason::OutsideManagedDirectory);
    }
    let metadata = fs::symlink_metadata(&candidate.path)
        .await
        .map_err(|_| JanitorSkipReason::ChangedDuringScan)?;
    if is_link_or_reparse(&metadata) {
        return Err(JanitorSkipReason::LinkOrReparsePoint);
    }
    if !metadata.is_file() {
        return Err(JanitorSkipReason::NonRegularFile);
    }
    let canonical = fs::canonicalize(&candidate.path)
        .await
        .map_err(|_| JanitorSkipReason::ChangedDuringScan)?;
    let modified = metadata
        .modified()
        .map_err(|_| JanitorSkipReason::ChangedDuringScan)?;
    if canonical != candidate.path
        || metadata.len() != candidate.size_bytes
        || modified != candidate.modified
        || !old_enough(now, modified, minimum_age)
    {
        return Err(JanitorSkipReason::ChangedDuringScan);
    }
    Ok(())
}

fn removal(candidate: &Candidate, reason: JanitorRemovalReason) -> JanitorRemoval {
    JanitorRemoval {
        path: candidate.path.clone(),
        size_bytes: candidate.size_bytes,
        reason,
    }
}

fn skip(report: &mut JanitorReport, path: PathBuf, reason: JanitorSkipReason) {
    report.skipped.push(JanitorSkip { path, reason });
}

fn is_managed_name(kind: ManagedKind, name: &OsStr) -> bool {
    let Some(name) = name.to_str() else {
        return false;
    };
    let identifier = match kind {
        ManagedKind::Staging => name.strip_suffix(".part"),
        ManagedKind::Object => Some(name),
    };
    identifier.is_some_and(|identifier| {
        identifier.len() == 32
            && identifier
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

pub(crate) fn is_link_or_reparse(metadata: &Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;

        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}
