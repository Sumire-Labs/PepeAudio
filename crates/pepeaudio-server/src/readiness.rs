use std::sync::Arc;

use pepeaudio_api::{BoxPortFuture, PortError, ReadinessProbe};
use pepeaudio_auth::{BotPresenceError, BotPresencePort, BoxAuthFuture, ValkeyAuthStore};
use pepeaudio_core::GuildId;
use pepeaudio_storage::{BotPresenceStore, PostgresStorage, ValkeyStore};

#[derive(Clone)]
pub(crate) struct PostgresReadiness(pub(crate) PostgresStorage);

impl ReadinessProbe for PostgresReadiness {
    fn ready(&self) -> BoxPortFuture<'_, Result<(), PortError>> {
        Box::pin(async move { self.0.ping().await.map_err(|_| PortError::Unavailable) })
    }
}

#[derive(Clone)]
pub(crate) struct AuthReadiness(pub(crate) ValkeyAuthStore);

impl ReadinessProbe for AuthReadiness {
    fn ready(&self) -> BoxPortFuture<'_, Result<(), PortError>> {
        Box::pin(async move { self.0.ping().await.map_err(|_| PortError::Unavailable) })
    }
}

#[derive(Clone)]
pub(crate) struct ValkeyBotPresence(pub(crate) ValkeyStore);

impl BotPresencePort for ValkeyBotPresence {
    fn is_present(&self, guild_id: GuildId) -> BoxAuthFuture<'_, Result<bool, BotPresenceError>> {
        Box::pin(async move {
            self.0
                .is_bot_present(guild_id)
                .await
                .map_err(|_| BotPresenceError::Unavailable)
        })
    }
}

pub(crate) fn shared_presence(store: ValkeyStore) -> Arc<dyn BotPresencePort> {
    Arc::new(ValkeyBotPresence(store))
}
