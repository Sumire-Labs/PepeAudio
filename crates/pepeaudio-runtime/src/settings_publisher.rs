use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use async_trait::async_trait;
use pepeaudio_core::{GuildId, PlayerSnapshot, StateRevision};
use pepeaudio_player::SnapshotPublisher;
use thiserror::Error;
use tokio::sync::watch;

use crate::settings_model::{PersistentPlayerSettings, SettingsUpdate, SettingsWorkerState};

/// Actor-side adapter which ignores initialization intermediates and queues
/// only changed durable controls.
pub struct SettingsSnapshotPublisher {
    pub(crate) guild_id: GuildId,
    pub(crate) initial: PersistentPlayerSettings,
    pub(crate) armed: bool,
    pub(crate) persist_initial: bool,
    pub(crate) last_accepted: PersistentPlayerSettings,
    pub(crate) mailbox: watch::Sender<SettingsWorkerState>,
    pub(crate) supervisor_closing: Arc<AtomicBool>,
    pub(crate) worker_closed: Arc<AtomicBool>,
}

#[async_trait]
impl SnapshotPublisher for SettingsSnapshotPublisher {
    type Error = SettingsPublishError;

    async fn publish(&mut self, snapshot: &PlayerSnapshot) -> Result<(), Self::Error> {
        if snapshot.guild_id != self.guild_id {
            return Err(SettingsPublishError::GuildMismatch);
        }
        if self.is_closed() {
            return Err(SettingsPublishError::SupervisorClosed);
        }
        let Some(settings) = PersistentPlayerSettings::from_snapshot(snapshot) else {
            return Ok(());
        };
        if !self.armed {
            if settings == self.initial {
                self.armed = true;
                self.last_accepted = settings.clone();
                if self.persist_initial {
                    self.queue(snapshot.revision, settings);
                    self.persist_initial = false;
                }
            }
            return Ok(());
        }
        if settings == self.last_accepted {
            return Ok(());
        }
        self.queue(snapshot.revision, settings.clone());
        self.last_accepted = settings;
        if self.is_closed() {
            Err(SettingsPublishError::SupervisorClosed)
        } else {
            Ok(())
        }
    }
}

impl SettingsSnapshotPublisher {
    fn queue(&self, actor_revision: StateRevision, settings: PersistentPlayerSettings) {
        let update = SettingsUpdate {
            actor_revision,
            settings,
        };
        self.mailbox.send_if_modified(|state| {
            if state
                .pending
                .as_ref()
                .is_none_or(|current| update.actor_revision > current.actor_revision)
            {
                state.pending = Some(update.clone());
                true
            } else {
                false
            }
        });
    }

    fn is_closed(&self) -> bool {
        self.supervisor_closing.load(Ordering::Acquire)
            || self.worker_closed.load(Ordering::Acquire)
            || self.mailbox.receiver_count() == 0
    }
}

/// Publishes a snapshot to the realtime snapshot path and durable settings path.
pub struct PersistentSnapshotPublisher<P> {
    primary: P,
    settings: SettingsSnapshotPublisher,
}

impl<P> PersistentSnapshotPublisher<P> {
    #[must_use]
    pub const fn new(primary: P, settings: SettingsSnapshotPublisher) -> Self {
        Self { primary, settings }
    }
}

#[async_trait]
impl<P> SnapshotPublisher for PersistentSnapshotPublisher<P>
where
    P: SnapshotPublisher,
{
    type Error = PersistentSnapshotPublishError;

    async fn publish(&mut self, snapshot: &PlayerSnapshot) -> Result<(), Self::Error> {
        let primary_failed = self.primary.publish(snapshot).await.is_err();
        let settings_failed = self.settings.publish(snapshot).await.is_err();
        match (primary_failed, settings_failed) {
            (false, false) => Ok(()),
            (true, false) => Err(PersistentSnapshotPublishError::Realtime),
            (false, true) => Err(PersistentSnapshotPublishError::Settings),
            (true, true) => Err(PersistentSnapshotPublishError::Both),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum SettingsPublishError {
    #[error("settings snapshot guild does not match its publisher")]
    GuildMismatch,
    #[error("settings persistence supervisor is closed")]
    SupervisorClosed,
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum PersistentSnapshotPublishError {
    #[error("realtime snapshot publication is unavailable")]
    Realtime,
    #[error("durable settings publication is unavailable")]
    Settings,
    #[error("snapshot publication destinations are unavailable")]
    Both,
}
