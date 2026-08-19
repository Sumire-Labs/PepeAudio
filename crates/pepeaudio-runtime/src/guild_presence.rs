use std::{collections::HashSet, sync::Arc, time::Duration};

use crate::{RuntimeError, RuntimeResult};
use futures_util::stream::{self, StreamExt as _};
use pepeaudio_core::GuildId;
use pepeaudio_storage::BotPresenceStore;
use tokio::{
    sync::{Semaphore, mpsc, oneshot},
    task::{JoinHandle, JoinSet},
    time::timeout,
};

const CLEAR_CONCURRENCY: usize = 32;
const CLEAR_TIMEOUT: Duration = Duration::from_secs(3);
const HEARTBEAT_CONCURRENCY: usize = 128;
const HEARTBEAT_REFRESH_TIMEOUT: Duration = Duration::from_secs(2);

const MAILBOX_CAPACITY: usize = 256;

enum PresenceCommand {
    Present(GuildId, oneshot::Sender<RuntimeResult<()>>),
    Absent(GuildId, oneshot::Sender<RuntimeResult<()>>),
    Shutdown(oneshot::Sender<()>),
}

#[derive(Clone)]
pub struct GuildPresenceHandle {
    sender: mpsc::Sender<PresenceCommand>,
}

impl GuildPresenceHandle {
    /// # Errors
    ///
    /// Returns when Valkey is unavailable or the presence runtime has stopped.
    pub async fn present(&self, guild_id: GuildId) -> RuntimeResult<()> {
        self.request(|reply| PresenceCommand::Present(guild_id, reply))
            .await
    }

    /// Removes a guild only while this process still owns its lease.
    ///
    /// # Errors
    ///
    /// Returns when Valkey is unavailable or the presence runtime has stopped.
    pub async fn absent(&self, guild_id: GuildId) -> RuntimeResult<()> {
        self.request(|reply| PresenceCommand::Absent(guild_id, reply))
            .await
    }

    async fn request(
        &self,
        build: impl FnOnce(oneshot::Sender<RuntimeResult<()>>) -> PresenceCommand,
    ) -> RuntimeResult<()> {
        let (reply, response) = oneshot::channel();
        self.sender
            .send(build(reply))
            .await
            .map_err(|_| RuntimeError::PresenceStopped)?;
        response.await.map_err(|_| RuntimeError::PresenceStopped)?
    }
}

/// Owner of expiring Valkey gateway-presence leases.
pub struct GuildPresenceRuntime {
    handle: GuildPresenceHandle,
    task: Option<JoinHandle<()>>,
}

impl GuildPresenceRuntime {
    /// # Errors
    ///
    /// Returns when lease timing or the instance identity is invalid.
    pub fn start<S>(
        store: S,
        instance_id: String,
        ttl: Duration,
        heartbeat: Duration,
    ) -> RuntimeResult<Self>
    where
        S: BotPresenceStore + Send + Sync + 'static,
    {
        if heartbeat.is_zero() || ttl.is_zero() || heartbeat >= ttl {
            return Err(RuntimeError::InvalidPresenceConfig);
        }
        if !valid_instance(&instance_id) {
            return Err(RuntimeError::InvalidPresenceConfig);
        }
        let (sender, receiver) = mpsc::channel(MAILBOX_CAPACITY);
        let handle = GuildPresenceHandle { sender };
        let task = tokio::spawn(run_actor(
            Arc::new(store),
            instance_id,
            ttl,
            heartbeat,
            receiver,
        ));
        Ok(Self {
            handle,
            task: Some(task),
        })
    }

    #[must_use]
    pub fn handle(&self) -> GuildPresenceHandle {
        self.handle.clone()
    }

    /// Waits for an uncoordinated actor exit, which supervisors must treat as
    /// fatal.
    pub async fn wait_for_unexpected_exit(&mut self) -> RuntimeError {
        let Some(task) = self.task.as_mut() else {
            return RuntimeError::RequiredTaskStopped {
                task: "guild presence",
            };
        };
        let result = task.await;
        self.task.take();
        match result {
            Ok(()) => RuntimeError::RequiredTaskStopped {
                task: "guild presence",
            },
            Err(error) => RuntimeError::Task(error),
        }
    }

    /// Clears all owned leases before joining the actor.
    ///
    /// # Errors
    ///
    /// Returns if the actor has stopped or its task fails.
    pub async fn shutdown(mut self) -> RuntimeResult<()> {
        let (reply, response) = oneshot::channel();
        self.handle
            .sender
            .send(PresenceCommand::Shutdown(reply))
            .await
            .map_err(|_| RuntimeError::PresenceStopped)?;
        response.await.map_err(|_| RuntimeError::PresenceStopped)?;
        if let Some(task) = self.task.take() {
            task.await.map_err(RuntimeError::Task)?;
        }
        Ok(())
    }
}

impl Drop for GuildPresenceRuntime {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

async fn run_actor<S>(
    store: Arc<S>,
    instance_id: String,
    ttl: Duration,
    heartbeat: Duration,
    mut receiver: mpsc::Receiver<PresenceCommand>,
) where
    S: BotPresenceStore + Send + Sync + 'static,
{
    let mut guilds = HashSet::new();
    let mut heartbeat_tasks = JoinSet::new();
    let mut interval = tokio::time::interval(heartbeat);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            biased;
            command = receiver.recv() => {
                let Some(command) = command else { break };
                match command {
                    PresenceCommand::Present(guild_id, reply) => {
                        // Desired ownership is retained even when the immediate
                        // write fails, so the heartbeat repairs a transient
                        // startup outage without waiting for another Gateway event.
                        guilds.insert(guild_id);
                        let result = match timeout(
                            HEARTBEAT_REFRESH_TIMEOUT,
                            store.refresh_bot_presence(guild_id, &instance_id, ttl),
                        )
                        .await
                        {
                            Ok(result) => result.map_err(RuntimeError::PresenceStore),
                            Err(_) => Err(RuntimeError::PresenceTimedOut {
                                operation: "present",
                            }),
                        };
                        let _ = reply.send(result);
                    }
                    PresenceCommand::Absent(guild_id, reply) => {
                        // Stop refreshing immediately. A failed clear is still
                        // safe because the remote lease expires at its TTL.
                        guilds.remove(&guild_id);
                        stop_heartbeat(&mut heartbeat_tasks).await;
                        let result = match timeout(
                            CLEAR_TIMEOUT,
                            store.clear_bot_presence(guild_id, &instance_id),
                        )
                        .await
                        {
                            Ok(result) => result
                                .map(|_cleared| ())
                                .map_err(RuntimeError::PresenceStore),
                            Err(_) => Err(RuntimeError::PresenceTimedOut {
                                operation: "absent",
                            }),
                        };
                        let _ = reply.send(result);
                    }
                    PresenceCommand::Shutdown(reply) => {
                        stop_heartbeat(&mut heartbeat_tasks).await;
                        clear_all(Arc::clone(&store), &instance_id, &guilds).await;
                        let _ = reply.send(());
                        return;
                    }
                }
            }
            result = heartbeat_tasks.join_next(), if !heartbeat_tasks.is_empty() => {
                if let Some(Err(error)) = result
                    && !error.is_cancelled()
                {
                    tracing::warn!(error = %error, "guild presence heartbeat task failed");
                }
            }
            _ = interval.tick(), if heartbeat_tasks.is_empty() => {
                let owned_guilds = guilds.iter().copied().collect();
                heartbeat_tasks.spawn(refresh_all(
                    Arc::clone(&store),
                    Arc::<str>::from(instance_id.clone()),
                    ttl,
                    owned_guilds,
                ));
            }
        }
    }
    stop_heartbeat(&mut heartbeat_tasks).await;
    clear_all(store, &instance_id, &guilds).await;
}

async fn refresh_all<S>(store: Arc<S>, instance_id: Arc<str>, ttl: Duration, guilds: Vec<GuildId>)
where
    S: BotPresenceStore + Send + Sync + 'static,
{
    stream::iter(guilds)
        .for_each_concurrent(Some(HEARTBEAT_CONCURRENCY), |guild_id| {
            let store = Arc::clone(&store);
            let instance_id = Arc::clone(&instance_id);
            async move {
                let refresh = timeout(
                    HEARTBEAT_REFRESH_TIMEOUT,
                    store.refresh_bot_presence(guild_id, &instance_id, ttl),
                )
                .await;
                match refresh {
                    Ok(Ok(())) => {}
                    Ok(Err(error)) => tracing::warn!(
                        guild_id = guild_id.get(),
                        error = %error,
                        "guild presence heartbeat failed; the desired lease will be retried"
                    ),
                    Err(_) => tracing::warn!(
                        guild_id = guild_id.get(),
                        timeout_ms = HEARTBEAT_REFRESH_TIMEOUT.as_millis(),
                        "guild presence heartbeat timed out; the desired lease will be retried"
                    ),
                }
            }
        })
        .await;
}

async fn stop_heartbeat(tasks: &mut JoinSet<()>) {
    tasks.abort_all();
    while let Some(result) = tasks.join_next().await {
        if let Err(error) = result
            && !error.is_cancelled()
        {
            tracing::warn!(error = %error, "guild presence heartbeat task failed");
        }
    }
}

async fn clear_all<S>(store: Arc<S>, instance_id: &str, guilds: &HashSet<GuildId>)
where
    S: BotPresenceStore + Send + Sync + 'static,
{
    let permits = Arc::new(Semaphore::new(CLEAR_CONCURRENCY));
    let mut tasks = JoinSet::new();
    for guild_id in guilds.iter().copied() {
        let store = Arc::clone(&store);
        let permits = Arc::clone(&permits);
        let instance_id = instance_id.to_owned();
        tasks.spawn(async move {
            let Ok(_permit) = permits.acquire_owned().await else {
                return;
            };
            let result = timeout(
                CLEAR_TIMEOUT,
                store.clear_bot_presence(guild_id, &instance_id),
            )
            .await;
            if !matches!(result, Ok(Ok(_))) {
                tracing::warn!(
                    guild_id = guild_id.get(),
                    "guild presence lease could not be cleared during shutdown"
                );
            }
        });
    }
    while let Some(result) = tasks.join_next().await {
        if let Err(error) = result {
            tracing::warn!(error = %error, "guild presence cleanup task failed");
        }
    }
}

fn valid_instance(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

#[cfg(test)]
#[path = "guild_presence_tests.rs"]
mod tests;
