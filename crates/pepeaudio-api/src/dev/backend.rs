use std::{
    collections::HashMap,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use pepeaudio_core::{
    CommandEnvelope, CommandResult, CommandValidationError, GuildId, PlayerCommand, PlayerSnapshot,
    StateRevision, UnixTimeMillis, UserId,
};
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::{
    BoxPortFuture, CommandReceipt, CommandResultSource, CommandRouter, PlayerEvent,
    PlayerEventSource, PortError, ReadinessProbe, RouteError, SnapshotSource,
};

use super::command_admission::CommandAdmission;

#[derive(Clone, Debug, Eq, PartialEq)]
struct LogicalCommand {
    guild_id: GuildId,
    actor_user_id: Option<UserId>,
    expected_revision: StateRevision,
    command: PlayerCommand,
}

impl From<&CommandEnvelope> for LogicalCommand {
    fn from(envelope: &CommandEnvelope) -> Self {
        Self {
            guild_id: envelope.guild_id,
            actor_user_id: envelope.actor_user_id,
            expected_revision: envelope.expected_revision,
            command: envelope.command.clone(),
        }
    }
}

struct BackendInner {
    command_admission: CommandAdmission,
    players: HashMap<GuildId, PlayerSnapshot>,
    receipts: HashMap<Uuid, (LogicalCommand, CommandReceipt)>,
    command_results: HashMap<Uuid, CommandResult>,
    senders: HashMap<GuildId, broadcast::Sender<PlayerEvent>>,
}

/// Non-durable single-process backend used by tests and explicit dev startup.
pub struct InMemoryPlayerBackend {
    inner: Mutex<BackendInner>,
    event_capacity: usize,
    ready: AtomicBool,
}

impl InMemoryPlayerBackend {
    #[must_use]
    pub fn new(snapshots: impl IntoIterator<Item = PlayerSnapshot>, event_capacity: usize) -> Self {
        let event_capacity = event_capacity.max(1);
        let mut players = HashMap::new();
        let mut senders = HashMap::new();
        for snapshot in snapshots {
            let guild_id = snapshot.guild_id;
            let (sender, _) = broadcast::channel(event_capacity);
            players.insert(guild_id, snapshot);
            senders.insert(guild_id, sender);
        }
        Self {
            inner: Mutex::new(BackendInner {
                command_admission: CommandAdmission::default(),
                players,
                receipts: HashMap::new(),
                command_results: HashMap::new(),
                senders,
            }),
            event_capacity,
            ready: AtomicBool::new(true),
        }
    }

    pub fn set_ready(&self, ready: bool) {
        self.ready.store(ready, Ordering::SeqCst);
    }

    /// # Errors
    ///
    /// Returns internal failure if the development lock is poisoned.
    pub fn publish_snapshot(&self, snapshot: PlayerSnapshot) -> Result<(), PortError> {
        let mut inner = self.inner.lock().map_err(|_| PortError::Internal)?;
        let guild_id = snapshot.guild_id;
        inner.players.insert(guild_id, snapshot.clone());
        let sender = inner.senders.entry(guild_id).or_insert_with(|| {
            let (sender, _) = broadcast::channel(self.event_capacity);
            sender
        });
        let _receiver_count = sender.send(PlayerEvent { snapshot });
        Ok(())
    }

    /// # Errors
    ///
    /// Returns internal failure if the development lock is poisoned.
    pub fn publish_command_result(&self, result: CommandResult) -> Result<(), PortError> {
        let mut inner = self.inner.lock().map_err(|_| PortError::Internal)?;
        inner.command_results.insert(result.command_id, result);
        Ok(())
    }
}

impl SnapshotSource for InMemoryPlayerBackend {
    fn snapshot(
        &self,
        guild_id: GuildId,
    ) -> BoxPortFuture<'_, Result<Option<PlayerSnapshot>, PortError>> {
        Box::pin(async move {
            self.inner
                .lock()
                .map_err(|_| PortError::Internal)
                .map(|inner| inner.players.get(&guild_id).cloned())
        })
    }
}

impl PlayerEventSource for InMemoryPlayerBackend {
    fn subscribe(&self, guild_id: GuildId) -> Result<broadcast::Receiver<PlayerEvent>, PortError> {
        let inner = self.inner.lock().map_err(|_| PortError::Internal)?;
        inner
            .senders
            .get(&guild_id)
            .map(broadcast::Sender::subscribe)
            .ok_or(PortError::NotFound)
    }
}

impl ReadinessProbe for InMemoryPlayerBackend {
    fn ready(&self) -> BoxPortFuture<'_, Result<(), PortError>> {
        Box::pin(async move {
            if self.ready.load(Ordering::SeqCst) {
                Ok(())
            } else {
                Err(PortError::Unavailable)
            }
        })
    }
}

impl CommandResultSource for InMemoryPlayerBackend {
    fn command_result(
        &self,
        guild_id: GuildId,
        command_id: Uuid,
    ) -> BoxPortFuture<'_, Result<Option<CommandResult>, PortError>> {
        Box::pin(async move {
            let inner = self.inner.lock().map_err(|_| PortError::Internal)?;
            Ok(inner
                .command_results
                .get(&command_id)
                .filter(|result| result.guild_id == guild_id)
                .cloned())
        })
    }
}

impl CommandRouter for InMemoryPlayerBackend {
    fn route(
        &self,
        envelope: CommandEnvelope,
        now: UnixTimeMillis,
    ) -> BoxPortFuture<'_, Result<CommandReceipt, RouteError>> {
        Box::pin(async move {
            let mut inner = self.inner.lock().map_err(|_| RouteError::Internal)?;
            inner
                .command_admission
                .admit(envelope.guild_id, envelope.actor_user_id, now)?;
            let logical = LogicalCommand::from(&envelope);
            if let Some((previous, receipt)) = inner.receipts.get(&envelope.idempotency_key) {
                if previous != &logical {
                    return Err(RouteError::IdempotencyConflict);
                }
                let mut replay = receipt.clone();
                replay.replayed = true;
                return Ok(replay);
            }
            let (published, changed) = {
                let snapshot = inner
                    .players
                    .get_mut(&envelope.guild_id)
                    .ok_or(RouteError::NotFound)?;
                envelope
                    .validate_against(snapshot, now)
                    .map_err(|error| map_validation_error(&error))?;
                let previous = snapshot.clone();
                super::player_commands::apply(snapshot, &envelope.command);
                let changed = !matches!(&envelope.command, PlayerCommand::MoveQueued { .. })
                    || *snapshot != previous;
                if changed {
                    snapshot.revision = previous
                        .revision
                        .checked_next()
                        .ok_or(RouteError::Internal)?;
                    snapshot.observed_at = now;
                }
                (snapshot.clone(), changed)
            };

            let receipt = CommandReceipt {
                command_id: envelope.command_id,
                idempotency_key: envelope.idempotency_key,
                resulting_revision: Some(published.revision),
                replayed: false,
            };
            inner.command_results.insert(
                receipt.command_id,
                CommandResult::applied(receipt.command_id, envelope.guild_id, published.revision),
            );
            inner
                .receipts
                .insert(envelope.idempotency_key, (logical, receipt.clone()));
            if changed && let Some(sender) = inner.senders.get(&envelope.guild_id) {
                let _receiver_count = sender.send(PlayerEvent {
                    snapshot: published,
                });
            }
            Ok(receipt)
        })
    }
}

fn map_validation_error(error: &CommandValidationError) -> RouteError {
    match error {
        CommandValidationError::RevisionConflict { expected, actual } => {
            RouteError::RevisionConflict {
                expected: *expected,
                actual: *actual,
            }
        }
        CommandValidationError::GuildMismatch { .. } => RouteError::Internal,
        CommandValidationError::DeadlineExpired { .. }
        | CommandValidationError::UnavailableInState { .. }
        | CommandValidationError::NoCurrentTrack { .. }
        | CommandValidationError::NoPreviousTrack
        | CommandValidationError::QueuedTrackNotFound { .. }
        | CommandValidationError::TrackNotSeekable
        | CommandValidationError::SeekPastEnd { .. } => RouteError::InvalidCommand,
    }
}
