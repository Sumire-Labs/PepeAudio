use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use pepeaudio_core::{GuildId, PlayerSnapshot, StateRevision};
use pepeaudio_storage::{SnapshotStore, SnapshotWrite};
use tokio::sync::watch;

#[derive(Clone, Copy)]
pub(super) struct SnapshotWorkerSettings {
    initial_backoff: Duration,
    maximum_backoff: Duration,
    write_timeout: Duration,
}

impl SnapshotWorkerSettings {
    #[cfg(test)]
    pub(super) const fn new(
        initial_backoff: Duration,
        maximum_backoff: Duration,
        write_timeout: Duration,
    ) -> Self {
        Self {
            initial_backoff,
            maximum_backoff,
            write_timeout,
        }
    }
}

impl Default for SnapshotWorkerSettings {
    fn default() -> Self {
        Self {
            initial_backoff: Duration::from_millis(100),
            maximum_backoff: Duration::from_secs(30),
            write_timeout: Duration::from_secs(10),
        }
    }
}

pub(super) async fn run_snapshot_worker<S>(
    guild_id: GuildId,
    store: Arc<S>,
    ttl: Duration,
    settings: SnapshotWorkerSettings,
    mut mailbox: watch::Receiver<Option<PlayerSnapshot>>,
    mut shutdown: watch::Receiver<bool>,
    closed: Arc<AtomicBool>,
) -> Result<(), SnapshotWorkerError>
where
    S: SnapshotStore + 'static,
{
    let result = run_worker(
        guild_id,
        store.as_ref(),
        ttl,
        settings,
        &mut mailbox,
        &mut shutdown,
    )
    .await;
    closed.store(true, Ordering::Release);
    tracing::debug!(
        guild_id = guild_id.get(),
        "snapshot publication worker stopped"
    );
    result
}

async fn run_worker<S>(
    guild_id: GuildId,
    store: &S,
    ttl: Duration,
    settings: SnapshotWorkerSettings,
    mailbox: &mut watch::Receiver<Option<PlayerSnapshot>>,
    shutdown: &mut watch::Receiver<bool>,
) -> Result<(), SnapshotWorkerError>
where
    S: SnapshotStore + 'static,
{
    let mut confirmed_revision = None;
    let mut backoff = settings.initial_backoff;
    let Some(mut snapshot) = wait_for_new_snapshot(mailbox, shutdown).await else {
        return flush_on_shutdown(
            guild_id,
            store,
            ttl,
            settings.write_timeout,
            mailbox,
            confirmed_revision,
            shutdown,
        )
        .await;
    };
    loop {
        if is_confirmed(snapshot.revision, confirmed_revision) {
            let Some(next) = wait_for_new_snapshot(mailbox, shutdown).await else {
                break;
            };
            snapshot = next;
            continue;
        }
        let write = store.put_snapshot_if_newer(&snapshot, ttl);
        let result = tokio::select! {
            biased;
            () = shutdown_signal(shutdown) => break,
            result = tokio::time::timeout(settings.write_timeout, write) => result,
        };
        match result {
            Ok(Ok(SnapshotWrite::Stored | SnapshotWrite::Stale)) => {
                confirmed_revision = Some(snapshot.revision);
                backoff = settings.initial_backoff;
                if let Some(latest) = latest_newer(mailbox, confirmed_revision) {
                    snapshot = latest;
                } else if let Some(next) = wait_for_new_snapshot(mailbox, shutdown).await {
                    snapshot = next;
                } else {
                    break;
                }
            }
            Ok(Err(_)) | Err(_) => {
                tracing::warn!(
                    guild_id = guild_id.get(),
                    revision = snapshot.revision.get(),
                    retry_after = ?backoff,
                    "snapshot storage write failed; retaining the latest revision for retry"
                );
                if !wait_for_retry(backoff, mailbox, shutdown).await {
                    break;
                }
                if let Some(latest) = mailbox.borrow_and_update().clone() {
                    snapshot = latest;
                }
                backoff = backoff.saturating_mul(2).min(settings.maximum_backoff);
            }
        }
    }
    flush_on_shutdown(
        guild_id,
        store,
        ttl,
        settings.write_timeout,
        mailbox,
        confirmed_revision,
        shutdown,
    )
    .await
}

async fn flush_on_shutdown<S>(
    guild_id: GuildId,
    store: &S,
    ttl: Duration,
    timeout: Duration,
    mailbox: &mut watch::Receiver<Option<PlayerSnapshot>>,
    confirmed_revision: Option<StateRevision>,
    shutdown: &watch::Receiver<bool>,
) -> Result<(), SnapshotWorkerError>
where
    S: SnapshotStore + 'static,
{
    if !*shutdown.borrow() {
        return Ok(());
    }
    let Some(snapshot) = latest_newer(mailbox, confirmed_revision) else {
        return Ok(());
    };
    match tokio::time::timeout(timeout, store.put_snapshot_if_newer(&snapshot, ttl)).await {
        Ok(Ok(SnapshotWrite::Stored | SnapshotWrite::Stale)) => Ok(()),
        Ok(Err(_)) => {
            tracing::error!(
                guild_id = guild_id.get(),
                revision = snapshot.revision.get(),
                "final snapshot storage write failed during shutdown"
            );
            Err(SnapshotWorkerError::FinalWriteFailed)
        }
        Err(_) => {
            tracing::error!(
                guild_id = guild_id.get(),
                revision = snapshot.revision.get(),
                timeout = ?timeout,
                "final snapshot storage write timed out during shutdown"
            );
            Err(SnapshotWorkerError::FinalWriteTimedOut)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SnapshotWorkerError {
    FinalWriteFailed,
    FinalWriteTimedOut,
}

fn is_confirmed(revision: StateRevision, confirmed: Option<StateRevision>) -> bool {
    confirmed.is_some_and(|value| revision <= value)
}

async fn wait_for_new_snapshot(
    mailbox: &mut watch::Receiver<Option<PlayerSnapshot>>,
    shutdown: &mut watch::Receiver<bool>,
) -> Option<PlayerSnapshot> {
    loop {
        if *shutdown.borrow() {
            return None;
        }
        tokio::select! {
            biased;
            () = shutdown_signal(shutdown) => return None,
            changed = mailbox.changed() => {
                changed.ok()?;
                if let Some(snapshot) = mailbox.borrow_and_update().clone() {
                    return Some(snapshot);
                }
            },
        }
    }
}

fn latest_newer(
    mailbox: &mut watch::Receiver<Option<PlayerSnapshot>>,
    confirmed: Option<StateRevision>,
) -> Option<PlayerSnapshot> {
    mailbox
        .borrow_and_update()
        .clone()
        .filter(|snapshot| !is_confirmed(snapshot.revision, confirmed))
}

async fn wait_for_retry(
    delay: Duration,
    mailbox: &mut watch::Receiver<Option<PlayerSnapshot>>,
    shutdown: &mut watch::Receiver<bool>,
) -> bool {
    let sleep = tokio::time::sleep(delay);
    tokio::pin!(sleep);
    loop {
        tokio::select! {
            biased;
            () = shutdown_signal(shutdown) => return false,
            () = &mut sleep => return true,
            changed = mailbox.changed() => {
                if changed.is_err() {
                    return false;
                }
                mailbox.borrow_and_update();
            }
        }
    }
}

async fn shutdown_signal(shutdown: &mut watch::Receiver<bool>) {
    loop {
        if *shutdown.borrow_and_update() {
            return;
        }
        if shutdown.changed().await.is_err() {
            return;
        }
    }
}
