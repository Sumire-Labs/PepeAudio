use std::{
    collections::HashMap,
    sync::{Arc, Weak},
};

use async_trait::async_trait;
use pepeaudio_core::GuildId;
use pepeaudio_player::{PlayerError, PlayerHandle, ShutdownReport};
use thiserror::Error;
use tokio::{
    sync::{Mutex, RwLock, Semaphore},
    task::JoinSet,
    time::{Duration, Instant, timeout, timeout_at},
};

const SHUTDOWN_CONCURRENCY: usize = 16;
const PLAYER_HEALTHCHECK_TIMEOUT: Duration = Duration::from_secs(2);
const PLAYER_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(15);
const PROCESS_SHUTDOWN_BUDGET: Duration = Duration::from_secs(20);

/// Factory boundary for creating a guild actor with its Songbird and snapshot adapters.
#[async_trait]
pub trait PlayerFactory: Send + Sync + 'static {
    async fn create(&self, guild_id: GuildId) -> Result<PlayerHandle, RegistryError>;
}

/// Concurrent guild-to-player handle registry with per-guild creation locking.
pub struct PlayerRegistry {
    factory: Arc<dyn PlayerFactory>,
    players: RwLock<HashMap<GuildId, PlayerHandle>>,
    operation_locks: Mutex<HashMap<GuildId, Weak<Mutex<()>>>>,
}

impl PlayerRegistry {
    #[must_use]
    pub fn new(factory: Arc<dyn PlayerFactory>) -> Self {
        Self {
            factory,
            players: RwLock::new(HashMap::new()),
            operation_locks: Mutex::new(HashMap::new()),
        }
    }

    pub async fn get(&self, guild_id: GuildId) -> Option<PlayerHandle> {
        self.live_player(guild_id).await.ok().flatten()
    }

    /// # Errors
    ///
    /// Returns a factory or registry error when the guild actor cannot start.
    pub async fn get_or_create(&self, guild_id: GuildId) -> Result<PlayerHandle, RegistryError> {
        let lock = self.operation_lock(guild_id).await;
        let _guard = lock.lock().await;
        if let Some(player) = self.live_player(guild_id).await? {
            return Ok(player);
        }
        self.players.write().await.remove(&guild_id);
        let player = self.factory.create(guild_id).await?;
        self.players.write().await.insert(guild_id, player.clone());
        Ok(player)
    }

    /// An actor which already completed its idle shutdown is also considered
    /// cleanly removed. The per-guild operation lock prevents a concurrent
    /// factory call from installing a replacement during permanent removal.
    ///
    /// # Errors
    ///
    /// Returns an actor shutdown error. When voice cleanup fails, the live
    /// actor is restored to the registry so a later request can retry cleanup.
    pub async fn remove_and_shutdown(&self, guild_id: GuildId) -> Result<bool, RegistryError> {
        let lock = self.operation_lock(guild_id).await;
        let _guard = lock.lock().await;
        let Some(player) = self.players.write().await.remove(&guild_id) else {
            return Ok(false);
        };
        match shutdown_player(&player).await {
            Ok(()) => Ok(true),
            Err(error) => {
                self.players.write().await.insert(guild_id, player);
                Err(error)
            }
        }
    }

    /// Cleanup continues after an individual failure so one guild cannot leave
    /// other voice or media resources alive during process shutdown.
    ///
    /// # Errors
    ///
    /// Returns the first shutdown error after all registered players have been
    /// given a cleanup attempt.
    pub async fn shutdown_all(&self) -> Result<(), RegistryError> {
        let players: Vec<_> = self.players.write().await.drain().collect();
        let permits = Arc::new(Semaphore::new(SHUTDOWN_CONCURRENCY));
        let mut tasks = JoinSet::new();
        for (guild_id, player) in players {
            let permits = Arc::clone(&permits);
            tasks.spawn(async move {
                let _permit = permits
                    .acquire_owned()
                    .await
                    .map_err(|_| RegistryError::Shutdown("cleanup coordinator closed".into()))?;
                shutdown_player(&player)
                    .await
                    .map_err(|error| RegistryError::Shutdown(format!("guild {guild_id}: {error}")))
            });
        }

        let deadline = Instant::now() + PROCESS_SHUTDOWN_BUDGET;
        let mut first_error = None;
        loop {
            match timeout_at(deadline, tasks.join_next()).await {
                Ok(Some(Ok(Ok(())))) => {}
                Ok(Some(Ok(Err(error)))) => {
                    first_error.get_or_insert(error);
                }
                Ok(Some(Err(error))) => {
                    first_error.get_or_insert_with(|| {
                        RegistryError::Shutdown(format!("cleanup task failed: {error}"))
                    });
                }
                Ok(None) => break,
                Err(_) => {
                    tasks.abort_all();
                    first_error.get_or_insert(RegistryError::Shutdown(
                        "player cleanup exceeded the process shutdown budget".into(),
                    ));
                    break;
                }
            }
        }
        while tasks.join_next().await.is_some() {}
        first_error.map_or(Ok(()), Err)
    }

    async fn operation_lock(&self, guild_id: GuildId) -> Arc<Mutex<()>> {
        let mut locks = self.operation_locks.lock().await;
        locks.retain(|_, lock| lock.strong_count() > 0);
        if let Some(lock) = locks.get(&guild_id).and_then(Weak::upgrade) {
            return lock;
        }
        let lock = Arc::new(Mutex::new(()));
        locks.insert(guild_id, Arc::downgrade(&lock));
        lock
    }

    async fn live_player(&self, guild_id: GuildId) -> Result<Option<PlayerHandle>, RegistryError> {
        let Some(player) = self.players.read().await.get(&guild_id).cloned() else {
            return Ok(None);
        };
        match timeout(PLAYER_HEALTHCHECK_TIMEOUT, player.snapshot()).await {
            Ok(Ok(_)) => Ok(Some(player)),
            Ok(Err(PlayerError::ActorStopped)) => Ok(None),
            Ok(Err(error)) => Err(RegistryError::Shutdown(error.to_string())),
            Err(_) => Err(RegistryError::ActorBusy),
        }
    }
}

async fn shutdown_player(player: &PlayerHandle) -> Result<(), RegistryError> {
    let result = timeout(PLAYER_SHUTDOWN_TIMEOUT, player.shutdown())
        .await
        .map_err(|_| RegistryError::Shutdown("player cleanup timed out".into()))?;
    match result {
        Ok(report) => completed_shutdown(report),
        Err(PlayerError::ActorStopped) => Ok(()),
        Err(error) => Err(RegistryError::Shutdown(error.to_string())),
    }
}

fn completed_shutdown(report: ShutdownReport) -> Result<(), RegistryError> {
    match report.disconnect_error {
        Some(error) => Err(RegistryError::Shutdown(error)),
        None => Ok(()),
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RegistryError {
    #[error("player actor did not answer its health check")]
    ActorBusy,
    #[error("player factory is not configured")]
    FactoryNotConfigured,
    #[error("player factory failed: {0}")]
    Factory(String),
    #[error("player shutdown failed: {0}")]
    Shutdown(String),
}

#[cfg(test)]
mod tests;
