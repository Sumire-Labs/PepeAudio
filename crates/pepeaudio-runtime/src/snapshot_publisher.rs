use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use async_trait::async_trait;
use pepeaudio_core::{GuildId, PlayerSnapshot, StateRevision};
use pepeaudio_player::SnapshotPublisher;
use pepeaudio_storage::SnapshotStore;
use thiserror::Error;
use tokio::{
    sync::{Mutex, RwLock, watch},
    task::JoinHandle,
};

use crate::snapshot_worker::{SnapshotWorkerError, SnapshotWorkerSettings, run_snapshot_worker};

/// Owns the self-healing snapshot workers and their explicit shutdown.
pub struct SnapshotPublisherRuntime<S> {
    shared: Arc<Shared<S>>,
}

impl<S> SnapshotPublisherRuntime<S>
where
    S: SnapshotStore + 'static,
{
    /// # Errors
    ///
    /// Returns [`SnapshotSupervisorError::InvalidTtl`] when `ttl` is zero.
    pub fn start(store: S, ttl: Duration) -> Result<Self, SnapshotSupervisorError> {
        Self::start_with_settings(store, ttl, SnapshotWorkerSettings::default())
    }

    pub(crate) fn start_with_settings(
        store: S,
        ttl: Duration,
        settings: SnapshotWorkerSettings,
    ) -> Result<Self, SnapshotSupervisorError> {
        if ttl.is_zero() {
            return Err(SnapshotSupervisorError::InvalidTtl);
        }
        Ok(Self {
            shared: Arc::new(Shared {
                store: Arc::new(store),
                ttl,
                settings,
                closing: Arc::new(AtomicBool::new(false)),
                publish_gate: Arc::new(RwLock::new(())),
                shutdown: watch::channel(false).0,
                workers: Mutex::new(HashMap::new()),
            }),
        })
    }

    #[must_use]
    pub fn handle(&self) -> SnapshotPublisherHandle<S> {
        SnapshotPublisherHandle {
            shared: Arc::clone(&self.shared),
        }
    }

    /// Gives every guild worker one bounded final write before closing.
    ///
    /// # Errors
    ///
    /// Returns when a worker panics or its final write fails or times out.
    pub async fn shutdown(self) -> Result<(), SnapshotSupervisorError> {
        self.shared.close_publishing();
        let publish_guard = self.shared.publish_gate.write().await;
        let tasks = {
            let mut workers = self.shared.workers.lock().await;
            workers
                .drain()
                .map(|(_, registration)| registration)
                .collect::<Vec<_>>()
        };
        self.shared.signal_shutdown();
        drop(publish_guard);
        let mut worker_panicked = false;
        let mut flush_failed = false;
        let mut flush_timed_out = false;
        for mut registration in tasks {
            match (&mut registration.task).await {
                Ok(Ok(())) => {}
                Ok(Err(SnapshotWorkerError::FinalWriteFailed)) => flush_failed = true,
                Ok(Err(SnapshotWorkerError::FinalWriteTimedOut)) => flush_timed_out = true,
                Err(_) => worker_panicked = true,
            }
        }
        if worker_panicked {
            Err(SnapshotSupervisorError::WorkerPanicked)
        } else if flush_timed_out {
            Err(SnapshotSupervisorError::FinalFlushTimedOut)
        } else if flush_failed {
            Err(SnapshotSupervisorError::FinalFlushFailed)
        } else {
            Ok(())
        }
    }
}

impl<S> Drop for SnapshotPublisherRuntime<S> {
    fn drop(&mut self) {
        self.shared.close_publishing();
        self.shared.signal_shutdown();
        if let Ok(mut workers) = self.shared.workers.try_lock() {
            for registration in workers.drain().map(|(_, registration)| registration) {
                registration.task.abort();
            }
        }
    }
}

pub struct SnapshotPublisherHandle<S> {
    shared: Arc<Shared<S>>,
}

impl<S> Clone for SnapshotPublisherHandle<S> {
    fn clone(&self) -> Self {
        Self {
            shared: Arc::clone(&self.shared),
        }
    }
}

impl<S> SnapshotPublisherHandle<S>
where
    S: SnapshotStore + 'static,
{
    /// Returns the newest revision accepted by this process for `guild_id`.
    ///
    /// Actor factories combine this volatile watermark with the durable
    /// watermark so an actor recreated during a Valkey outage cannot roll its
    /// revision back behind a coalesced snapshot that is still retrying.
    pub async fn latest_revision(&self, guild_id: GuildId) -> Option<StateRevision> {
        let workers = self.shared.workers.lock().await;
        workers
            .get(&guild_id)
            .and_then(WorkerRegistration::latest_revision)
    }

    /// # Errors
    ///
    /// Returns [`SnapshotPublishError::SupervisorClosed`] after shutdown begins.
    pub async fn publisher(
        &self,
        guild_id: GuildId,
    ) -> Result<ValkeySnapshotPublisher, SnapshotPublishError> {
        if self.shared.is_closing() {
            return Err(SnapshotPublishError::SupervisorClosed);
        }
        let mut workers = self.shared.workers.lock().await;
        if self.shared.is_closing() {
            return Err(SnapshotPublishError::SupervisorClosed);
        }
        if let Some(registration) = workers.get(&guild_id) {
            return Ok(registration.publisher(
                guild_id,
                &self.shared.closing,
                &self.shared.publish_gate,
            ));
        }

        let (mailbox, receiver) = watch::channel(None);
        let worker_closed = Arc::new(AtomicBool::new(false));
        let task = tokio::spawn(run_snapshot_worker(
            guild_id,
            Arc::clone(&self.shared.store),
            self.shared.ttl,
            self.shared.settings,
            receiver,
            self.shared.shutdown.subscribe(),
            Arc::clone(&worker_closed),
        ));
        let registration = WorkerRegistration {
            mailbox,
            worker_closed,
            task,
        };
        let publisher =
            registration.publisher(guild_id, &self.shared.closing, &self.shared.publish_gate);
        workers.insert(guild_id, registration);
        Ok(publisher)
    }
}

/// Non-blocking actor adapter backed by a capacity-one latest-wins mailbox.
pub struct ValkeySnapshotPublisher {
    guild_id: GuildId,
    mailbox: watch::Sender<Option<PlayerSnapshot>>,
    supervisor_closing: Arc<AtomicBool>,
    publish_gate: Arc<RwLock<()>>,
    worker_closed: Arc<AtomicBool>,
}

#[async_trait]
impl SnapshotPublisher for ValkeySnapshotPublisher {
    type Error = SnapshotPublishError;

    async fn publish(&mut self, snapshot: &PlayerSnapshot) -> Result<(), Self::Error> {
        if snapshot.guild_id != self.guild_id {
            return Err(SnapshotPublishError::GuildMismatch);
        }
        let _publish_guard = self.publish_gate.read().await;
        if self.is_closed() {
            return Err(SnapshotPublishError::SupervisorClosed);
        }
        self.mailbox.send_if_modified(|latest| {
            if latest
                .as_ref()
                .is_none_or(|queued| snapshot.revision > queued.revision)
            {
                *latest = Some(snapshot.clone());
                true
            } else {
                false
            }
        });
        if self.worker_is_closed() {
            Err(SnapshotPublishError::SupervisorClosed)
        } else {
            Ok(())
        }
    }
}

impl ValkeySnapshotPublisher {
    fn is_closed(&self) -> bool {
        self.supervisor_closing.load(Ordering::Acquire) || self.worker_is_closed()
    }

    fn worker_is_closed(&self) -> bool {
        self.worker_closed.load(Ordering::Acquire) || self.mailbox.receiver_count() == 0
    }
}

struct Shared<S> {
    store: Arc<S>,
    ttl: Duration,
    settings: SnapshotWorkerSettings,
    closing: Arc<AtomicBool>,
    publish_gate: Arc<RwLock<()>>,
    shutdown: watch::Sender<bool>,
    workers: Mutex<HashMap<GuildId, WorkerRegistration>>,
}

impl<S> Shared<S> {
    fn close_publishing(&self) {
        self.closing.store(true, Ordering::Release);
    }

    fn signal_shutdown(&self) {
        self.shutdown.send_replace(true);
    }

    fn is_closing(&self) -> bool {
        self.closing.load(Ordering::Acquire)
    }
}

struct WorkerRegistration {
    mailbox: watch::Sender<Option<PlayerSnapshot>>,
    worker_closed: Arc<AtomicBool>,
    task: JoinHandle<Result<(), SnapshotWorkerError>>,
}

impl Drop for WorkerRegistration {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl WorkerRegistration {
    fn latest_revision(&self) -> Option<StateRevision> {
        self.mailbox
            .borrow()
            .as_ref()
            .map(|snapshot| snapshot.revision)
    }

    fn publisher(
        &self,
        guild_id: GuildId,
        supervisor_closing: &Arc<AtomicBool>,
        publish_gate: &Arc<RwLock<()>>,
    ) -> ValkeySnapshotPublisher {
        ValkeySnapshotPublisher {
            guild_id,
            mailbox: self.mailbox.clone(),
            supervisor_closing: Arc::clone(supervisor_closing),
            publish_gate: Arc::clone(publish_gate),
            worker_closed: Arc::clone(&self.worker_closed),
        }
    }
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum SnapshotPublishError {
    #[error("snapshot guild does not match its publisher")]
    GuildMismatch,
    #[error("snapshot publication supervisor is closed")]
    SupervisorClosed,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum SnapshotSupervisorError {
    #[error("snapshot TTL must be greater than zero")]
    InvalidTtl,
    #[error("a snapshot publication worker panicked")]
    WorkerPanicked,
    #[error("a final snapshot write failed during shutdown")]
    FinalFlushFailed,
    #[error("a final snapshot write timed out during shutdown")]
    FinalFlushTimedOut,
}
