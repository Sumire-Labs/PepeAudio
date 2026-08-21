use std::{
    future::Future,
    pin::Pin,
    time::{Duration, SystemTime},
};

use pepeaudio_core::{
    CommandEnvelope, CommandResult, GuildId, PlayerSnapshot, StateRevision, UnixTimeMillis,
};
use serde::Serialize;
use thiserror::Error;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::{HrirPresetCatalog, Principal};

/// Heap-erased future used by object-safe async application ports.
pub type BoxPortFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Player operation whose permission must be evaluated at request time.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Access {
    ReadPlayer,
    ControlPlayer,
    SubscribeEvents,
}

pub trait SnapshotSource: Send + Sync {
    /// Returns `None` before the guild has produced its first durable snapshot.
    fn snapshot(
        &self,
        guild_id: GuildId,
    ) -> BoxPortFuture<'_, Result<Option<PlayerSnapshot>, PortError>>;
}

pub trait HrirPresetCatalogSource: Send + Sync {
    /// Lists global and guild-owned presets visible to `guild_id`.
    fn hrir_presets(
        &self,
        guild_id: GuildId,
    ) -> BoxPortFuture<'_, Result<HrirPresetCatalog, PortError>>;
}

/// Command delivery port. The owner must preserve idempotency and revision
/// checks; API-side checks are never a substitute for atomic owner validation.
pub trait CommandRouter: Send + Sync {
    fn route(
        &self,
        envelope: CommandEnvelope,
        now: UnixTimeMillis,
    ) -> BoxPortFuture<'_, Result<CommandReceipt, RouteError>>;
}

pub trait CommandResultSource: Send + Sync {
    /// Returns `None` after the bounded result retention window expires.
    fn command_result(
        &self,
        guild_id: GuildId,
        command_id: Uuid,
    ) -> BoxPortFuture<'_, Result<Option<CommandResult>, PortError>>;
}

/// Subscription port backed by a bounded broadcast channel or equivalent.
pub trait PlayerEventSource: Send + Sync {
    /// Creates a new receiver before the initial full snapshot is fetched.
    ///
    /// # Errors
    ///
    /// Returns a safe transport failure when the guild stream cannot be opened.
    fn subscribe(&self, guild_id: GuildId) -> Result<broadcast::Receiver<PlayerEvent>, PortError>;
}

/// Request-time authorization policy shared by read, mutation, and SSE paths.
pub trait Authorizer: Send + Sync {
    fn authorize<'a>(
        &'a self,
        principal: &'a Principal,
        guild_id: GuildId,
        access: Access,
    ) -> BoxPortFuture<'a, Result<(), AuthorizationError>>;
}

/// Dependency readiness probe. Liveness deliberately does not call this port.
pub trait ReadinessProbe: Send + Sync {
    fn ready(&self) -> BoxPortFuture<'_, Result<(), PortError>>;
}

/// Clock port used to make command deadlines deterministic in tests.
pub trait Clock: Send + Sync {
    /// Current Unix timestamp in milliseconds.
    fn now(&self) -> UnixTimeMillis;
}

/// Host wall clock. Backwards jumps remain visible; player idle timers must use
/// a separate monotonic clock in their owning runtime.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> UnixTimeMillis {
        let millis = SystemTime::UNIX_EPOCH
            .elapsed()
            .map_or(0, |duration| duration.as_millis());
        UnixTimeMillis::new(u64::try_from(millis).unwrap_or(u64::MAX))
    }
}

/// One versioned state notification. The SSE adapter still sends a full
/// snapshot first and treats gaps as a resynchronization boundary.
#[derive(Clone, Debug)]
pub struct PlayerEvent {
    pub snapshot: PlayerSnapshot,
}

/// Stable acknowledgement returned from command routing.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CommandReceipt {
    pub command_id: Uuid,
    /// Logical retry key supplied by the client.
    pub idempotency_key: Uuid,
    /// Revision after application, when the owner returned it synchronously.
    pub resulting_revision: Option<StateRevision>,
    /// Whether this response reuses an earlier result for the same logical key.
    pub replayed: bool,
}

/// Shared backend failure classes that are safe to map without leaking detail.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum PortError {
    #[error("player not found")]
    NotFound,
    #[error("service dependency unavailable")]
    Unavailable,
    #[error("internal backend failure")]
    Internal,
}

/// Owner-side command rejection classes.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RouteError {
    #[error("player not found")]
    NotFound,
    #[error("expected revision {expected:?}, current revision {actual:?}")]
    RevisionConflict {
        expected: StateRevision,
        actual: StateRevision,
    },
    #[error("command rejected by the player owner")]
    InvalidCommand,
    #[error("idempotency key was reused with a different request")]
    IdempotencyConflict,
    #[error("player command rate limit exceeded; retry after {retry_after:?}")]
    RateLimited {
        /// Server-authoritative delay before another attempt.
        retry_after: Duration,
    },
    #[error("command routing unavailable")]
    Unavailable,
    #[error("internal command routing failure")]
    Internal,
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum AuthorizationError {
    #[error("guild access forbidden")]
    Forbidden,
    #[error("authorization policy unavailable")]
    Unavailable,
}
