use std::{future::Future, pin::Pin};

use pepeaudio_core::{GuildId, UserId};
use thiserror::Error;

use crate::{OAuthProjection, PendingOAuth, SessionData};

/// Heap-erased future used by object-safe authentication ports.
pub type BoxAuthFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Discord OAuth code exchange followed by identity and guild projection.
pub trait OAuthProvider: Send + Sync {
    /// Exchanges the code using the exact redirect URI and PKCE verifier.
    /// OAuth access and refresh tokens must be dropped before this returns.
    fn exchange_projection<'a>(
        &'a self,
        code: &'a str,
        verifier: &'a str,
    ) -> BoxAuthFuture<'a, Result<OAuthProjection, OAuthProviderError>>;
}

/// One-time pending OAuth state repository.
pub trait PendingOAuthStore: Send + Sync {
    /// Reserves a random state with `SET NX` semantics.
    fn reserve<'a>(
        &'a self,
        state: &'a str,
        pending: PendingOAuth,
    ) -> BoxAuthFuture<'a, Result<(), RepositoryError>>;

    /// Atomically consumes a state. Replays return `None`.
    fn consume<'a>(
        &'a self,
        state: &'a str,
    ) -> BoxAuthFuture<'a, Result<Option<PendingOAuth>, RepositoryError>>;
}

/// Full opaque-session repository used by handlers and API adapters.
pub trait OpaqueSessionRepository: Send + Sync {
    /// Creates a session and returns its raw cookie token exactly once.
    fn create(&self, session: SessionData) -> BoxAuthFuture<'_, Result<String, RepositoryError>>;

    /// Resolves and sliding-refreshes an opaque token.
    fn load<'a>(
        &'a self,
        opaque_token: &'a str,
    ) -> BoxAuthFuture<'a, Result<Option<SessionData>, RepositoryError>>;

    /// Resolves a session only when its fingerprint still owns the user's
    /// current-session pointer. The raw cookie is never required here.
    fn load_bound<'a>(
        &'a self,
        user_id: UserId,
        session_fingerprint: &'a str,
    ) -> BoxAuthFuture<'a, Result<Option<SessionData>, RepositoryError>>;

    /// Invalidates a session and its current-user pointer when it still owns it.
    fn destroy<'a>(
        &'a self,
        opaque_token: &'a str,
    ) -> BoxAuthFuture<'a, Result<(), RepositoryError>>;
}

/// Current bot membership, supplied by the gateway owner or a bounded cache.
pub trait BotPresencePort: Send + Sync {
    fn is_present(&self, guild_id: GuildId) -> BoxAuthFuture<'_, Result<bool, BotPresenceError>>;
}

/// Clock abstraction for deterministic expiry tests.
pub trait AuthClock: Send + Sync {
    /// Unix time in milliseconds.
    ///
    /// # Errors
    ///
    /// Returns an error if the host clock cannot produce a Unix timestamp.
    fn now_ms(&self) -> Result<u64, ClockError>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SystemAuthClock;

impl AuthClock for SystemAuthClock {
    fn now_ms(&self) -> Result<u64, ClockError> {
        crate::crypto::unix_millis().map_err(|_| ClockError::Unavailable)
    }
}

/// OAuth provider failure with no token or response-body detail.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum OAuthProviderError {
    #[error("OAuth request was rejected")]
    Rejected,
    #[error("OAuth provider unavailable")]
    Unavailable,
    #[error("OAuth provider returned invalid data")]
    InvalidResponse,
}

/// Valkey session/state failure safe for policy mapping.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum RepositoryError {
    #[error("opaque authentication credential is invalid")]
    InvalidToken,
    #[error("authentication repository unavailable")]
    Unavailable,
    #[error("authentication repository contained invalid data")]
    Corrupt,
    #[error("authentication repository rejected a duplicate token")]
    Collision,
    /// Deployment-wide pending OAuth admission is full.
    #[error("authentication repository capacity is exhausted")]
    CapacityExceeded,
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum BotPresenceError {
    #[error("bot presence unavailable")]
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ClockError {
    #[error("host clock unavailable")]
    Unavailable,
}

#[derive(Clone, Debug, Default)]
pub struct FixedBotPresence {
    guilds: std::collections::HashSet<GuildId>,
}

impl FixedBotPresence {
    #[must_use]
    pub fn new(guilds: impl IntoIterator<Item = GuildId>) -> Self {
        Self {
            guilds: guilds.into_iter().collect(),
        }
    }
}

impl BotPresencePort for FixedBotPresence {
    fn is_present(&self, guild_id: GuildId) -> BoxAuthFuture<'_, Result<bool, BotPresenceError>> {
        Box::pin(async move { Ok(self.guilds.contains(&guild_id)) })
    }
}
