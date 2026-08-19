use std::{
    collections::HashSet,
    num::NonZeroU32,
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use futures_util::StreamExt as _;
use pepeaudio_api::{
    BoxPortFuture, CommandReceipt, CommandResultSource, CommandRouter, PlayerEventSource,
    PortError, ReadinessProbe, RouteError, SnapshotSource,
};
use pepeaudio_core::{CommandEnvelope, CommandResult, GuildId, PlayerSnapshot, UnixTimeMillis};
use pepeaudio_storage::{
    CommandProducer, CommandResultStore, SnapshotEventStream, SnapshotEventSubscriber,
    SnapshotStore, StorageError,
};
use tokio::sync::watch;

use crate::{
    ApiBackendRuntime, DEFAULT_COMMAND_RESULT_RETENTION, RuntimeError, RuntimeResult,
    event_hub::EventHub,
};

/// Full snapshots are always fetched from the versioned cache. Pub/Sub only
/// wakes the bounded local fan-out and is never treated as authoritative state.
pub struct ValkeyApiBackend<S> {
    store: S,
    shard_total: NonZeroU32,
    events: EventHub,
    subscribed_guilds: RwLock<HashSet<GuildId>>,
    subscription_ready: AtomicBool,
}

impl<S> ValkeyApiBackend<S>
where
    S: SnapshotStore + SnapshotEventSubscriber + CommandProducer + Clone + Send + Sync + 'static,
{
    /// Opens the initial subscription before returning. Later disconnects are
    /// retried in the background.
    ///
    /// # Errors
    ///
    /// Returns when the initial Valkey subscription cannot be opened.
    pub async fn start(
        store: S,
        shard_total: NonZeroU32,
        event_capacity: usize,
        retry_delay: Duration,
    ) -> RuntimeResult<(Arc<Self>, ApiBackendRuntime)> {
        let stream = store
            .subscribe_snapshot_events()
            .await
            .map_err(RuntimeError::EventSubscription)?;
        let backend = Arc::new(Self {
            store,
            shard_total,
            events: EventHub::new(event_capacity),
            subscribed_guilds: RwLock::new(HashSet::new()),
            subscription_ready: AtomicBool::new(true),
        });
        let (shutdown, receiver) = watch::channel(false);
        let task_backend = Arc::clone(&backend);
        let task = tokio::spawn(async move {
            task_backend
                .run_events(stream, receiver, retry_delay.max(Duration::from_millis(10)))
                .await;
        });
        Ok((
            backend,
            ApiBackendRuntime {
                shutdown: Some(shutdown),
                task: Some(task),
            },
        ))
    }

    async fn run_events(
        self: Arc<Self>,
        mut stream: SnapshotEventStream,
        mut shutdown: watch::Receiver<bool>,
        retry_delay: Duration,
    ) {
        loop {
            let disconnected = loop {
                tokio::select! {
                    changed = shutdown.changed() => {
                        if changed.is_err() || *shutdown.borrow() {
                            self.subscription_ready.store(false, Ordering::Release);
                            return;
                        }
                    }
                    notification = stream.next() => {
                        match notification {
                            Some(Ok(event)) => {
                                match self.store.get_snapshot(event.guild_id).await {
                                    Ok(Some(snapshot)) if snapshot.revision >= event.revision => {
                                        self.events.publish(snapshot);
                                    }
                                    Ok(_) => {}
                                    Err(error) if is_snapshot_data_error(&error) => {
                                        tracing::warn!(
                                            guild_id = %event.guild_id,
                                            "ignored an invalid cached player snapshot"
                                        );
                                    }
                                    Err(_) => break true,
                                }
                            }
                            Some(Err(_)) | None => break true,
                        }
                    }
                }
            };
            if disconnected {
                self.subscription_ready.store(false, Ordering::Release);
            }

            loop {
                tokio::select! {
                    changed = shutdown.changed() => {
                        if changed.is_err() || *shutdown.borrow() {
                            return;
                        }
                    }
                    () = tokio::time::sleep(retry_delay) => {
                        if let Ok(new_stream) = self.store.subscribe_snapshot_events().await {
                            stream = new_stream;
                            if self.publish_current_subscriber_snapshots().await {
                                self.subscription_ready.store(true, Ordering::Release);
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    async fn publish_current_subscriber_snapshots(&self) -> bool {
        let guilds = self
            .subscribed_guilds
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .copied()
            .collect::<Vec<_>>();
        for guild_id in guilds {
            match self.store.get_snapshot(guild_id).await {
                Ok(Some(snapshot)) if snapshot.guild_id == guild_id => {
                    self.events.publish(snapshot);
                }
                Ok(None) => {}
                Ok(Some(_)) => {
                    tracing::warn!(
                        guild_id = %guild_id,
                        "ignored a cached player snapshot with the wrong guild identity"
                    );
                }
                Err(error) if is_snapshot_data_error(&error) => {
                    tracing::warn!(
                        guild_id = %guild_id,
                        "ignored an invalid cached player snapshot during subscription recovery"
                    );
                }
                Err(_) => return false,
            }
        }
        true
    }
}

fn is_snapshot_data_error(error: &StorageError) -> bool {
    matches!(
        error,
        StorageError::Json(_) | StorageError::CorruptData { .. }
    )
}

impl<S> SnapshotSource for ValkeyApiBackend<S>
where
    S: SnapshotStore + Send + Sync,
{
    fn snapshot(
        &self,
        guild_id: GuildId,
    ) -> BoxPortFuture<'_, Result<Option<PlayerSnapshot>, PortError>> {
        Box::pin(async move {
            self.store
                .get_snapshot(guild_id)
                .await
                .map_err(|_| PortError::Unavailable)
        })
    }
}

impl<S> CommandRouter for ValkeyApiBackend<S>
where
    S: CommandProducer + Send + Sync,
{
    fn route(
        &self,
        envelope: CommandEnvelope,
        now: UnixTimeMillis,
    ) -> BoxPortFuture<'_, Result<CommandReceipt, RouteError>> {
        Box::pin(async move {
            if now >= envelope.deadline {
                return Err(RouteError::InvalidCommand);
            }
            if envelope.actor_user_id.is_none() {
                return Err(RouteError::InvalidCommand);
            }
            let command_id = envelope.command_id;
            let idempotency_key = envelope.idempotency_key;
            match self
                .store
                .enqueue_command(
                    &envelope,
                    self.shard_total.get(),
                    DEFAULT_COMMAND_RESULT_RETENTION,
                )
                .await
            {
                Ok(_) => {}
                Err(StorageError::RateLimited { retry_after, .. }) => {
                    return Err(RouteError::RateLimited { retry_after });
                }
                Err(_) => return Err(RouteError::Unavailable),
            }
            Ok(CommandReceipt {
                command_id,
                idempotency_key,
                resulting_revision: None,
                replayed: false,
            })
        })
    }
}

impl<S> CommandResultSource for ValkeyApiBackend<S>
where
    S: CommandResultStore + Send + Sync,
{
    fn command_result(
        &self,
        guild_id: GuildId,
        command_id: uuid::Uuid,
    ) -> BoxPortFuture<'_, Result<Option<CommandResult>, PortError>> {
        Box::pin(async move {
            self.store
                .get_command_result(guild_id, command_id)
                .await
                .map_err(|_| PortError::Unavailable)
        })
    }
}

impl<S> PlayerEventSource for ValkeyApiBackend<S>
where
    S: Send + Sync,
{
    fn subscribe(
        &self,
        guild_id: GuildId,
    ) -> Result<tokio::sync::broadcast::Receiver<pepeaudio_api::PlayerEvent>, PortError> {
        self.subscribed_guilds
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(guild_id);
        Ok(self.events.subscribe(guild_id))
    }
}

impl<S> ReadinessProbe for ValkeyApiBackend<S>
where
    S: SnapshotStore + Send + Sync,
{
    fn ready(&self) -> BoxPortFuture<'_, Result<(), PortError>> {
        Box::pin(async move {
            if !self.subscription_ready.load(Ordering::Acquire) {
                return Err(PortError::Unavailable);
            }
            match self
                .store
                .get_snapshot(GuildId::new(1).expect("constant guild ID is valid"))
                .await
            {
                Ok(_) => Ok(()),
                Err(error) if is_snapshot_data_error(&error) => Ok(()),
                Err(_) => Err(PortError::Unavailable),
            }
        })
    }
}

#[cfg(test)]
#[path = "api_backend_tests.rs"]
mod tests;
