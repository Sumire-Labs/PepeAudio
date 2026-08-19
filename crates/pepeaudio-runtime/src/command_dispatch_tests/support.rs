use std::{
    collections::HashMap,
    future::{Future, pending},
    sync::{Arc, Mutex},
    time::Duration,
};

use async_trait::async_trait;
use pepeaudio_core::{
    CommandEnvelope, CommandResult, CommandResultStatus, GuildId, PlayerCommand, StateRevision,
    UnixTimeMillis, UserId, Volume,
};
use pepeaudio_player::PlayerHandle;
use pepeaudio_storage::{
    ClaimBatch, CommandCompletion, CommandConsumer, CommandResultStore, CommandResultWrite,
    DedupeClaim, IdempotencyStore, ReceivedCommand, StorageResult,
};
use tokio::{
    sync::{Notify, watch},
    task::JoinHandle,
};
use uuid::Uuid;

use crate::{
    CommandAuthorization, CommandAuthorizer, CommandWorkerConfig, PlayerDirectory,
    WorkerPlayerError, command_dispatch::run_shard,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum StoreEvent {
    Result(Uuid),
    Acknowledged(String),
}

#[derive(Clone)]
pub(super) struct DispatchStore {
    state: Arc<Mutex<StoreState>>,
    changed: Arc<Notify>,
}

struct StoreState {
    first_read: Option<Vec<ReceivedCommand>>,
    results: HashMap<Uuid, CommandResult>,
    events: Vec<StoreEvent>,
}

impl DispatchStore {
    pub(super) fn new(commands: Vec<ReceivedCommand>) -> Self {
        let results = commands
            .iter()
            .map(|received| {
                let envelope = &received.envelope;
                (
                    envelope.command_id,
                    CommandResult::pending(envelope.command_id, envelope.guild_id),
                )
            })
            .collect();
        Self {
            state: Arc::new(Mutex::new(StoreState {
                first_read: Some(commands),
                results,
                events: Vec::new(),
            })),
            changed: Arc::new(Notify::new()),
        }
    }

    pub(super) fn events(&self) -> Vec<StoreEvent> {
        self.state.lock().expect("store lock").events.clone()
    }

    pub(super) fn result_status(&self, command_id: Uuid) -> CommandResultStatus {
        self.state
            .lock()
            .expect("store lock")
            .results
            .get(&command_id)
            .expect("known command result")
            .status
    }

    pub(super) async fn wait_for_acknowledgement(&self, entry_id: &str) {
        loop {
            let changed = self.changed.notified();
            if self
                .events()
                .iter()
                .any(|event| matches!(event, StoreEvent::Acknowledged(id) if id == entry_id))
            {
                return;
            }
            changed.await;
        }
    }

    fn write_result(&self, result: &CommandResult) -> CommandResultWrite {
        let mut state = self.state.lock().expect("store lock");
        let write = match state.results.get(&result.command_id) {
            Some(current) if current == result => CommandResultWrite::Unchanged,
            Some(current) if current.is_terminal() => CommandResultWrite::Conflict,
            Some(_) => CommandResultWrite::Stored,
            None => CommandResultWrite::Conflict,
        };
        if write == CommandResultWrite::Stored {
            state.results.insert(result.command_id, result.clone());
            state.events.push(StoreEvent::Result(result.command_id));
            self.changed.notify_waiters();
        }
        write
    }
}

#[async_trait]
impl CommandConsumer for DispatchStore {
    async fn ensure_command_group(&self, _: u32, _: &str) -> StorageResult<()> {
        Ok(())
    }

    async fn read_commands(
        &self,
        _: u32,
        _: &str,
        _: &str,
        _: usize,
        _: Duration,
    ) -> StorageResult<Vec<ReceivedCommand>> {
        let first_read = self.state.lock().expect("store lock").first_read.take();
        match first_read {
            Some(commands) => Ok(commands),
            None => pending().await,
        }
    }

    async fn claim_abandoned_commands(
        &self,
        _: u32,
        _: &str,
        _: &str,
        _: Duration,
        _: &str,
        _: usize,
    ) -> StorageResult<ClaimBatch> {
        Ok(ClaimBatch {
            next_start: "0-0".into(),
            commands: Vec::new(),
            deleted_entry_ids: Vec::new(),
        })
    }

    async fn acknowledge_command(&self, _: u32, _: &str, entry_id: &str) -> StorageResult<bool> {
        self.state
            .lock()
            .expect("store lock")
            .events
            .push(StoreEvent::Acknowledged(entry_id.into()));
        self.changed.notify_waiters();
        Ok(true)
    }
}

#[async_trait]
impl CommandResultStore for DispatchStore {
    async fn get_command_result(
        &self,
        guild_id: GuildId,
        command_id: Uuid,
    ) -> StorageResult<Option<CommandResult>> {
        Ok(self
            .state
            .lock()
            .expect("store lock")
            .results
            .get(&command_id)
            .filter(|result| result.guild_id == guild_id)
            .cloned())
    }

    async fn store_command_result(
        &self,
        result: &CommandResult,
        _: Duration,
    ) -> StorageResult<CommandResultWrite> {
        Ok(self.write_result(result))
    }

    async fn complete_command(
        &self,
        _: Uuid,
        _: Uuid,
        result: &CommandResult,
        _: Duration,
        _: Duration,
    ) -> StorageResult<CommandCompletion> {
        Ok(match self.write_result(result) {
            CommandResultWrite::Stored => CommandCompletion::Stored,
            CommandResultWrite::Unchanged => CommandCompletion::Unchanged,
            CommandResultWrite::Conflict => CommandCompletion::Conflict,
        })
    }
}

#[async_trait]
impl IdempotencyStore for DispatchStore {
    async fn claim_idempotency(
        &self,
        _: GuildId,
        _: Uuid,
        _: Uuid,
        _: Duration,
    ) -> StorageResult<DedupeClaim> {
        Ok(DedupeClaim::Acquired)
    }

    async fn complete_idempotency(
        &self,
        _: GuildId,
        _: Uuid,
        _: Uuid,
        _: Duration,
    ) -> StorageResult<bool> {
        Ok(true)
    }

    async fn release_idempotency(&self, _: GuildId, _: Uuid, _: Uuid) -> StorageResult<bool> {
        Ok(true)
    }
}

struct EmptyDirectory;

#[async_trait]
impl PlayerDirectory for EmptyDirectory {
    async fn player(&self, _: GuildId) -> Result<Option<PlayerHandle>, WorkerPlayerError> {
        Ok(None)
    }
}

pub(super) struct LaneAuthorizer {
    slow_id: Uuid,
    second_id: Uuid,
    slow_entered: Notify,
    second_entered: Notify,
    release_slow: Notify,
    observed: Mutex<Vec<Uuid>>,
}

impl LaneAuthorizer {
    pub(super) fn new(slow_id: Uuid, second_id: Uuid) -> Arc<Self> {
        Arc::new(Self {
            slow_id,
            second_id,
            slow_entered: Notify::new(),
            second_entered: Notify::new(),
            release_slow: Notify::new(),
            observed: Mutex::new(Vec::new()),
        })
    }

    pub(super) async fn wait_for_slow(&self) {
        self.slow_entered.notified().await;
    }

    pub(super) async fn wait_for_second(&self) {
        self.second_entered.notified().await;
    }

    pub(super) fn release_slow(&self) {
        self.release_slow.notify_one();
    }

    pub(super) fn observed(&self) -> Vec<Uuid> {
        self.observed.lock().expect("authorizer lock").clone()
    }
}

#[async_trait]
impl CommandAuthorizer for LaneAuthorizer {
    async fn authorize(&self, envelope: &CommandEnvelope) -> CommandAuthorization {
        self.observed
            .lock()
            .expect("authorizer lock")
            .push(envelope.command_id);
        if envelope.command_id == self.slow_id {
            self.slow_entered.notify_one();
            self.release_slow.notified().await;
        } else if envelope.command_id == self.second_id {
            self.second_entered.notify_one();
        }
        CommandAuthorization::Denied
    }
}

pub(super) fn spawn_worker(
    store: DispatchStore,
    authorizer: Arc<LaneAuthorizer>,
) -> (watch::Sender<bool>, JoinHandle<()>) {
    let (shutdown, receiver) = watch::channel(false);
    let worker = tokio::spawn(run_shard(
        store,
        Arc::new(EmptyDirectory),
        authorizer,
        config(),
        0,
        receiver,
    ));
    (shutdown, worker)
}

pub(super) async fn within<F>(future: F, message: &'static str)
where
    F: Future<Output = ()>,
{
    tokio::time::timeout(Duration::from_secs(1), future)
        .await
        .expect(message);
}

fn config() -> CommandWorkerConfig {
    CommandWorkerConfig {
        shards: 0..1,
        group: "dispatch-test".into(),
        consumer: "worker-test".into(),
        batch_size: 16,
        block: Duration::from_secs(30),
        claim_idle: Duration::from_mins(1),
        lease_ttl: Duration::from_secs(90),
        completion_retention: Duration::from_mins(10),
        retry_delay: Duration::from_millis(10),
    }
}

pub(super) fn slow_id() -> Uuid {
    Uuid::from_u128(1)
}

pub(super) fn second_id() -> Uuid {
    Uuid::from_u128(2)
}

pub(super) fn fast_id() -> Uuid {
    Uuid::from_u128(3)
}

pub(super) fn guild(value: u64) -> GuildId {
    GuildId::new(value).expect("guild")
}

pub(super) fn command(entry_id: &str, command_id: Uuid, guild_id: GuildId) -> ReceivedCommand {
    ReceivedCommand {
        entry_id: entry_id.into(),
        envelope: CommandEnvelope {
            command_id,
            idempotency_key: Uuid::from_u128(command_id.as_u128() + 100),
            guild_id,
            actor_user_id: Some(UserId::new(99).expect("user")),
            expected_revision: StateRevision::INITIAL,
            deadline: UnixTimeMillis::new(u64::MAX),
            command: PlayerCommand::SetVolume {
                volume: Volume::new(50).expect("volume"),
            },
        },
    }
}
