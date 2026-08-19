use std::{
    collections::HashMap,
    panic::AssertUnwindSafe,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use futures_util::FutureExt as _;
use pepeaudio_core::GuildId;
use pepeaudio_storage::{GuildSettings, GuildSettingsRepository};
use thiserror::Error;
use tokio::{
    sync::{Mutex, mpsc, watch},
    task::JoinHandle,
};

use crate::{
    settings_model::{PersistentPlayerSettings, SettingsPersistenceView, SettingsWorkerState},
    settings_publisher::{SettingsPublishError, SettingsSnapshotPublisher},
    settings_worker::{SettingsWorkerConfig, SettingsWorkerError, run_settings_worker},
};

/// Owns non-blocking, latest-wins `PostgreSQL` persistence workers per guild.
pub struct SettingsPersistenceRuntime<R> {
    shared: Arc<Shared<R>>,
    failures: mpsc::UnboundedReceiver<SettingsSupervisorError>,
}

impl<R> SettingsPersistenceRuntime<R>
where
    R: GuildSettingsRepository + 'static,
{
    #[must_use]
    pub fn start(repository: R) -> Self {
        Self::start_with_config(repository, SettingsWorkerConfig::default())
    }

    pub(crate) fn start_with_config(repository: R, config: SettingsWorkerConfig) -> Self {
        let (failure_sender, failures) = mpsc::unbounded_channel();
        Self {
            shared: Arc::new(Shared {
                repository: Arc::new(repository),
                config,
                closing: Arc::new(AtomicBool::new(false)),
                shutdown: watch::channel(false).0,
                workers: Mutex::new(HashMap::new()),
                failure_sender,
            }),
            failures,
        }
    }

    #[must_use]
    pub fn handle(&self) -> SettingsPersistenceHandle<R> {
        SettingsPersistenceHandle {
            shared: Arc::clone(&self.shared),
        }
    }

    /// Waits for an uncoordinated worker exit. This is fatal because future
    /// player mutations would have no durable settings path.
    pub async fn wait_for_unexpected_exit(&mut self) -> SettingsSupervisorError {
        self.failures
            .recv()
            .await
            .unwrap_or(SettingsSupervisorError::MonitorClosed)
    }

    /// Flushes the latest accepted values before joining every guild worker.
    ///
    /// # Errors
    ///
    /// Returns when a final write fails or a worker panics.
    pub async fn shutdown(self) -> Result<(), SettingsSupervisorError> {
        self.shared.begin_shutdown();
        let tasks = {
            let mut workers = self.shared.workers.lock().await;
            workers
                .drain()
                .map(|(_, registration)| registration)
                .collect::<Vec<_>>()
        };
        let mut first_error = None;
        for mut registration in tasks {
            match (&mut registration.task).await {
                Ok(Err(error)) if first_error.is_none() => first_error = Some(error),
                Err(_) if first_error.is_none() => {
                    first_error = Some(SettingsSupervisorError::MonitorPanicked);
                }
                Ok(Ok(()) | Err(_)) | Err(_) => {}
            }
        }
        first_error.map_or(Ok(()), Err)
    }
}

impl<R> Drop for SettingsPersistenceRuntime<R> {
    fn drop(&mut self) {
        self.shared.begin_shutdown();
        if let Ok(mut workers) = self.shared.workers.try_lock() {
            for registration in workers.drain().map(|(_, registration)| registration) {
                registration.task.abort();
            }
        }
    }
}

pub struct SettingsPersistenceHandle<R> {
    shared: Arc<Shared<R>>,
}

impl<R> Clone for SettingsPersistenceHandle<R> {
    fn clone(&self) -> Self {
        Self {
            shared: Arc::clone(&self.shared),
        }
    }
}

impl<R> SettingsPersistenceHandle<R>
where
    R: GuildSettingsRepository + 'static,
{
    pub async fn latest(&self, guild_id: GuildId) -> Option<SettingsPersistenceView> {
        self.shared
            .workers
            .lock()
            .await
            .get(&guild_id)
            .map(WorkerRegistration::view)
    }

    /// Seeds newly created publishers with the current durable row.
    ///
    /// # Errors
    ///
    /// Returns after shutdown begins or when `seed` belongs to another guild.
    pub async fn publisher(
        &self,
        guild_id: GuildId,
        seed: GuildSettings,
        initial: PersistentPlayerSettings,
    ) -> Result<SettingsSnapshotPublisher, SettingsPublishError> {
        if seed.guild_id != guild_id {
            return Err(SettingsPublishError::GuildMismatch);
        }
        if self.shared.is_closing() {
            return Err(SettingsPublishError::SupervisorClosed);
        }
        let mut workers = self.shared.workers.lock().await;
        if self.shared.is_closing() {
            return Err(SettingsPublishError::SupervisorClosed);
        }
        if let Some(registration) = workers.get(&guild_id) {
            let view = registration.view();
            let current = view
                .pending
                .or_else(|| PersistentPlayerSettings::from_guild_settings(&view.durable));
            let persist_initial = current.as_ref() != Some(&initial);
            return Ok(registration.publisher(
                guild_id,
                initial,
                persist_initial,
                &self.shared.closing,
            ));
        }

        let persist_initial =
            PersistentPlayerSettings::from_guild_settings(&seed).as_ref() != Some(&initial);
        let (mailbox, receiver) = watch::channel(SettingsWorkerState {
            durable: seed.clone(),
            pending: None,
        });
        let worker_closed = Arc::new(AtomicBool::new(false));
        let task_closed = Arc::clone(&worker_closed);
        let worker_mailbox = mailbox.clone();
        let repository = Arc::clone(&self.shared.repository);
        let config = self.shared.config;
        let shutdown = self.shared.shutdown.subscribe();
        let closing = Arc::clone(&self.shared.closing);
        let failure_sender = self.shared.failure_sender.clone();
        let task = tokio::spawn(async move {
            let worker = AssertUnwindSafe(run_settings_worker(
                guild_id,
                repository,
                seed,
                worker_mailbox,
                receiver,
                shutdown,
                config,
            ))
            .catch_unwind()
            .await;
            let result = match worker {
                Ok(Ok(())) if closing.load(Ordering::Acquire) => Ok(()),
                Ok(Ok(())) => Err(SettingsSupervisorError::WorkerStopped { guild_id }),
                Ok(Err(SettingsWorkerError::FinalWriteFailed)) => {
                    Err(SettingsSupervisorError::FinalWriteFailed { guild_id })
                }
                Ok(Err(SettingsWorkerError::Repository | SettingsWorkerError::Conflict)) => {
                    Err(SettingsSupervisorError::WorkerStopped { guild_id })
                }
                Err(_) => Err(SettingsSupervisorError::WorkerPanicked { guild_id }),
            };
            task_closed.store(true, Ordering::Release);
            if let Err(error) = result
                && !closing.load(Ordering::Acquire)
            {
                let _ignored = failure_sender.send(error);
            }
            result
        });
        let registration = WorkerRegistration {
            mailbox,
            worker_closed,
            task,
        };
        let publisher =
            registration.publisher(guild_id, initial, persist_initial, &self.shared.closing);
        workers.insert(guild_id, registration);
        Ok(publisher)
    }
}

struct Shared<R> {
    repository: Arc<R>,
    config: SettingsWorkerConfig,
    closing: Arc<AtomicBool>,
    shutdown: watch::Sender<bool>,
    workers: Mutex<HashMap<GuildId, WorkerRegistration>>,
    failure_sender: mpsc::UnboundedSender<SettingsSupervisorError>,
}

impl<R> Shared<R> {
    fn begin_shutdown(&self) {
        self.closing.store(true, Ordering::Release);
        self.shutdown.send_replace(true);
    }

    fn is_closing(&self) -> bool {
        self.closing.load(Ordering::Acquire)
    }
}

struct WorkerRegistration {
    mailbox: watch::Sender<SettingsWorkerState>,
    worker_closed: Arc<AtomicBool>,
    task: JoinHandle<Result<(), SettingsSupervisorError>>,
}

impl Drop for WorkerRegistration {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl WorkerRegistration {
    fn view(&self) -> SettingsPersistenceView {
        self.mailbox.borrow().view()
    }

    fn publisher(
        &self,
        guild_id: GuildId,
        initial: PersistentPlayerSettings,
        persist_initial: bool,
        supervisor_closing: &Arc<AtomicBool>,
    ) -> SettingsSnapshotPublisher {
        SettingsSnapshotPublisher {
            guild_id,
            initial: initial.clone(),
            armed: false,
            persist_initial,
            last_accepted: initial,
            mailbox: self.mailbox.clone(),
            supervisor_closing: Arc::clone(supervisor_closing),
            worker_closed: Arc::clone(&self.worker_closed),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum SettingsSupervisorError {
    #[error("the durable settings failure monitor stopped")]
    MonitorClosed,
    #[error("the durable settings monitor panicked")]
    MonitorPanicked,
    #[error("durable settings worker for guild {guild_id} stopped unexpectedly")]
    WorkerStopped { guild_id: GuildId },
    #[error("durable settings worker for guild {guild_id} could not flush its final value")]
    FinalWriteFailed { guild_id: GuildId },
    #[error("durable settings worker for guild {guild_id} panicked")]
    WorkerPanicked { guild_id: GuildId },
}
