use std::{
    path::PathBuf,
    time::{Duration, SystemTime},
};

use super::{JanitorPolicy, JanitorRemovalReason};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ManagedKind {
    Staging,
    Object,
}

#[derive(Debug)]
pub(super) struct Candidate {
    pub(super) path: PathBuf,
    pub(super) kind: ManagedKind,
    pub(super) size_bytes: u64,
    pub(super) modified: SystemTime,
    pub(super) leased: bool,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Plan {
    pub(super) index: usize,
    pub(super) reason: JanitorRemovalReason,
}

pub(super) fn plan_removals(
    candidates: &[Candidate],
    policy: JanitorPolicy,
    now: SystemTime,
    observed_bytes: u64,
) -> Vec<Plan> {
    let mut plans = Vec::new();
    let mut planned = vec![false; candidates.len()];
    let mut retained = observed_bytes;

    for (index, candidate) in candidates.iter().enumerate() {
        if candidate.leased {
            continue;
        }
        let reason = match candidate.kind {
            ManagedKind::Staging if old_enough(now, candidate.modified, policy.staging_ttl) => {
                Some(JanitorRemovalReason::StagingExpired)
            }
            ManagedKind::Object if old_enough(now, candidate.modified, policy.object_ttl) => {
                Some(JanitorRemovalReason::ObjectExpired)
            }
            ManagedKind::Staging | ManagedKind::Object => None,
        };
        if let Some(reason) = reason {
            plans.push(Plan { index, reason });
            planned[index] = true;
            retained = retained.saturating_sub(candidate.size_bytes);
        }
    }

    let mut quota_candidates: Vec<_> = candidates
        .iter()
        .enumerate()
        .filter(|(index, candidate)| {
            !planned[*index]
                && !candidate.leased
                && candidate.kind == ManagedKind::Object
                && old_enough(now, candidate.modified, policy.minimum_object_retention)
        })
        .collect();
    quota_candidates.sort_by(|left, right| {
        left.1
            .modified
            .cmp(&right.1.modified)
            .then_with(|| left.1.path.cmp(&right.1.path))
    });
    for (index, candidate) in quota_candidates {
        if retained <= policy.max_total_bytes {
            break;
        }
        plans.push(Plan {
            index,
            reason: JanitorRemovalReason::Capacity,
        });
        retained = retained.saturating_sub(candidate.size_bytes);
    }
    plans
}

pub(super) fn old_enough(now: SystemTime, modified: SystemTime, minimum_age: Duration) -> bool {
    now.duration_since(modified)
        .is_ok_and(|age| age >= minimum_age)
}
