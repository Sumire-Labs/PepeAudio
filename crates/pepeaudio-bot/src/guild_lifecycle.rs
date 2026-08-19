use std::{collections::HashSet, sync::Arc, time::Duration};

use async_trait::async_trait;
use pepeaudio_core::GuildId;
use pepeaudio_runtime::GuildPresenceHandle;
use pepeaudio_storage::{SnapshotStore, ValkeyStore};
use thiserror::Error;
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinHandle,
};

use crate::guild_lifecycle_actor::run_actor;

const MAILBOX_CAPACITY: usize = 256;

#[derive(Clone, Copy)]
pub(super) struct RetryPolicy {
    pub(super) short_delays: [Duration; 2],
    pub(super) background_interval: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            short_delays: [Duration::from_millis(40), Duration::from_millis(120)],
            background_interval: Duration::from_secs(2),
        }
    }
}

#[async_trait]
pub(super) trait SnapshotInvalidator: Send + Sync {
    async fn invalidate(&self, guild_id: GuildId) -> Result<(), GuildLifecycleError>;
}

#[async_trait]
impl SnapshotInvalidator for ValkeyStore {
    async fn invalidate(&self, guild_id: GuildId) -> Result<(), GuildLifecycleError> {
        self.invalidate_snapshot(guild_id)
            .await
            .map_err(|_| GuildLifecycleError)
    }
}

pub(super) enum LifecycleCommand {
    ReconcileShard {
        shard_id: u32,
        guilds: HashSet<GuildId>,
        reply: oneshot::Sender<ShardReconciliation>,
    },
    Present {
        shard_id: u32,
        guild_id: GuildId,
        reply: oneshot::Sender<Result<(), GuildLifecycleError>>,
    },
    Absent {
        shard_id: u32,
        guild_id: GuildId,
        reply: oneshot::Sender<GuildAbsence>,
    },
    Owned {
        shard_id: u32,
        reply: oneshot::Sender<Vec<GuildId>>,
    },
    Shutdown(oneshot::Sender<()>),
}

pub(crate) struct ShardReconciliation {
    pub(crate) removed: Vec<GuildId>,
    pub(crate) update: Result<(), GuildLifecycleError>,
}

pub(crate) struct GuildAbsence {
    pub(crate) no_longer_owned: bool,
    pub(crate) update: Result<(), GuildLifecycleError>,
}

/// Cloneable, ordered Gateway guild-lifecycle command handle.
#[derive(Clone)]
pub struct GuildLifecycleHandle {
    sender: mpsc::Sender<LifecycleCommand>,
}

impl GuildLifecycleHandle {
    pub(crate) async fn reconcile_shard(
        &self,
        shard_id: u32,
        guilds: HashSet<GuildId>,
    ) -> Result<ShardReconciliation, GuildLifecycleError> {
        let (reply, response) = oneshot::channel();
        self.sender
            .send(LifecycleCommand::ReconcileShard {
                shard_id,
                guilds,
                reply,
            })
            .await
            .map_err(|_| GuildLifecycleError)?;
        response.await.map_err(|_| GuildLifecycleError)
    }

    /// Invalidates the disposable snapshot before advertising ownership.
    ///
    /// A transient failure leaves the desired state in the lifecycle actor;
    /// background reconciliation continues without another Gateway event.
    ///
    /// # Errors
    ///
    /// Fails closed when the immediate invalidation or presence write cannot
    /// be confirmed, or when the lifecycle actor has stopped.
    pub async fn present_on_shard(
        &self,
        shard_id: u32,
        guild_id: GuildId,
    ) -> Result<(), GuildLifecycleError> {
        let (reply, response) = oneshot::channel();
        self.sender
            .send(LifecycleCommand::Present {
                shard_id,
                guild_id,
                reply,
            })
            .await
            .map_err(|_| GuildLifecycleError)?;
        response.await.map_err(|_| GuildLifecycleError)?
    }

    pub(crate) async fn remove_from_shard(
        &self,
        shard_id: u32,
        guild_id: GuildId,
    ) -> Result<GuildAbsence, GuildLifecycleError> {
        let (reply, response) = oneshot::channel();
        self.sender
            .send(LifecycleCommand::Absent {
                shard_id,
                guild_id,
                reply,
            })
            .await
            .map_err(|_| GuildLifecycleError)?;
        response.await.map_err(|_| GuildLifecycleError)
    }

    /// Removes this shard's desired ownership and stops its presence heartbeat.
    ///
    /// # Errors
    ///
    /// Returns when the immediate cleanup cannot be confirmed or the actor has
    /// stopped. A remote lease still expires at its bounded TTL.
    pub async fn absent_on_shard(
        &self,
        shard_id: u32,
        guild_id: GuildId,
    ) -> Result<(), GuildLifecycleError> {
        self.remove_from_shard(shard_id, guild_id).await?.update
    }

    /// Returns the latest READY ownership set for one shard.
    pub async fn owned_on_shard(&self, shard_id: u32) -> Vec<GuildId> {
        let (reply, response) = oneshot::channel();
        if self
            .sender
            .send(LifecycleCommand::Owned { shard_id, reply })
            .await
            .is_err()
        {
            return Vec::new();
        }
        response.await.unwrap_or_default()
    }
}

pub(crate) struct GuildLifecycleRuntime {
    handle: GuildLifecycleHandle,
    task: Option<JoinHandle<()>>,
}

impl GuildLifecycleRuntime {
    pub(crate) fn start(valkey: ValkeyStore, presence: GuildPresenceHandle) -> Self {
        Self::start_with(Arc::new(valkey), presence, RetryPolicy::default())
    }

    fn start_with(
        snapshots: Arc<dyn SnapshotInvalidator>,
        presence: GuildPresenceHandle,
        retry: RetryPolicy,
    ) -> Self {
        let (sender, receiver) = mpsc::channel(MAILBOX_CAPACITY);
        let handle = GuildLifecycleHandle { sender };
        let task = tokio::spawn(run_actor(snapshots, presence, retry, receiver));
        Self {
            handle,
            task: Some(task),
        }
    }

    #[must_use]
    pub(crate) fn handle(&self) -> GuildLifecycleHandle {
        self.handle.clone()
    }

    pub(crate) async fn wait_for_unexpected_exit(&mut self) -> GuildLifecycleTaskError {
        let Some(task) = self.task.as_mut() else {
            return GuildLifecycleTaskError::Stopped;
        };
        let result = task.await;
        self.task.take();
        match result {
            Ok(()) => GuildLifecycleTaskError::Stopped,
            Err(error) => GuildLifecycleTaskError::Task(error),
        }
    }

    pub(crate) async fn shutdown(mut self) -> Result<(), GuildLifecycleError> {
        let (reply, response) = oneshot::channel();
        self.handle
            .sender
            .send(LifecycleCommand::Shutdown(reply))
            .await
            .map_err(|_| GuildLifecycleError)?;
        response.await.map_err(|_| GuildLifecycleError)?;
        if let Some(task) = self.task.take() {
            task.await.map_err(|_| GuildLifecycleError)?;
        }
        Ok(())
    }
}

impl Drop for GuildLifecycleRuntime {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

#[derive(Debug, Error)]
pub(crate) enum GuildLifecycleTaskError {
    #[error("guild lifecycle actor stopped unexpectedly")]
    Stopped,
    #[error("guild lifecycle actor task failed")]
    Task(#[source] tokio::task::JoinError),
}

#[derive(Clone, Copy, Debug, Error)]
#[error("guild lifecycle state is temporarily unavailable")]
pub struct GuildLifecycleError;

#[cfg(test)]
#[path = "guild_lifecycle_tests.rs"]
mod tests;
