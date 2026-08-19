use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use async_trait::async_trait;
use pepeaudio_core::{
    CommandEnvelope, CommandResult, GuildId, PlayerCommand, StateRevision, UnixTimeMillis, UserId,
    Volume,
};
use pepeaudio_player::PlayerHandle;
use pepeaudio_storage::{
    ClaimBatch, CommandCompletion, CommandConsumer, CommandResultStore, CommandResultWrite,
    DedupeClaim, IdempotencyStore, ReceivedCommand, StorageError, StorageResult,
};
use uuid::Uuid;

use crate::{
    CommandAuthorization, CommandAuthorizer, CommandWorkerConfig, PlayerDirectory,
    WorkerPlayerError, command_loop::process_command,
};

#[derive(Clone)]
pub(super) struct TestStore {
    pub(super) state: Arc<Mutex<StoreState>>,
}

pub(super) struct StoreState {
    pub(super) claim_outcome: DedupeClaim,
    pub(super) claims: usize,
    pub(super) completions: usize,
    pub(super) releases: usize,
    pub(super) acknowledgements: Vec<String>,
    pub(super) result: CommandResult,
    pub(super) result_writes: usize,
    fail_result_writes: bool,
}

impl TestStore {
    pub(super) fn new(claim_outcome: DedupeClaim) -> Self {
        Self {
            state: Arc::new(Mutex::new(StoreState {
                claim_outcome,
                claims: 0,
                completions: 0,
                releases: 0,
                acknowledgements: Vec::new(),
                result: CommandResult::pending(
                    Uuid::from_u128(1),
                    GuildId::new(10).expect("guild"),
                ),
                result_writes: 0,
                fail_result_writes: false,
            })),
        }
    }

    pub(super) fn fail_result_writes(&self) {
        self.state.lock().expect("store lock").fail_result_writes = true;
    }
}

#[async_trait]
impl CommandConsumer for TestStore {
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
        Ok(Vec::new())
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
            .acknowledgements
            .push(entry_id.into());
        Ok(true)
    }
}

#[async_trait]
impl IdempotencyStore for TestStore {
    async fn claim_idempotency(
        &self,
        _: GuildId,
        _: Uuid,
        _: Uuid,
        _: Duration,
    ) -> StorageResult<DedupeClaim> {
        let mut state = self.state.lock().expect("store lock");
        state.claims += 1;
        Ok(state.claim_outcome)
    }

    async fn complete_idempotency(
        &self,
        _: GuildId,
        _: Uuid,
        _: Uuid,
        _: Duration,
    ) -> StorageResult<bool> {
        self.state.lock().expect("store lock").completions += 1;
        Ok(true)
    }

    async fn release_idempotency(&self, _: GuildId, _: Uuid, _: Uuid) -> StorageResult<bool> {
        self.state.lock().expect("store lock").releases += 1;
        Ok(true)
    }
}

#[async_trait]
impl CommandResultStore for TestStore {
    async fn get_command_result(
        &self,
        guild_id: GuildId,
        command_id: Uuid,
    ) -> StorageResult<Option<CommandResult>> {
        let state = self.state.lock().expect("store lock");
        Ok(
            (state.result.guild_id == guild_id && state.result.command_id == command_id)
                .then(|| state.result.clone()),
        )
    }

    async fn store_command_result(
        &self,
        result: &CommandResult,
        _: Duration,
    ) -> StorageResult<CommandResultWrite> {
        let mut state = self.state.lock().expect("store lock");
        state.result_writes += 1;
        if state.fail_result_writes {
            return Err(result_write_error());
        }
        Ok(write_result(&mut state.result, result))
    }

    async fn complete_command(
        &self,
        _: Uuid,
        _: Uuid,
        result: &CommandResult,
        _: Duration,
        _: Duration,
    ) -> StorageResult<CommandCompletion> {
        let mut state = self.state.lock().expect("store lock");
        state.completions += 1;
        state.result_writes += 1;
        if state.fail_result_writes {
            return Err(result_write_error());
        }
        Ok(match write_result(&mut state.result, result) {
            CommandResultWrite::Stored => CommandCompletion::Stored,
            CommandResultWrite::Unchanged => CommandCompletion::Unchanged,
            CommandResultWrite::Conflict => CommandCompletion::Conflict,
        })
    }
}

fn write_result(current: &mut CommandResult, result: &CommandResult) -> CommandResultWrite {
    if current == result {
        CommandResultWrite::Unchanged
    } else if current.is_terminal() {
        CommandResultWrite::Conflict
    } else {
        current.clone_from(result);
        CommandResultWrite::Stored
    }
}

const fn result_write_error() -> StorageError {
    StorageError::CorruptData {
        entity: "test command result",
        field: "forced_failure",
    }
}

pub(super) struct TestDirectory {
    pub(super) player: Option<PlayerHandle>,
    pub(super) lookups: AtomicUsize,
}

#[async_trait]
impl PlayerDirectory for TestDirectory {
    async fn player(&self, _: GuildId) -> Result<Option<PlayerHandle>, WorkerPlayerError> {
        self.lookups.fetch_add(1, Ordering::SeqCst);
        Ok(self.player.clone())
    }
}

pub(super) struct TestAuthorizer {
    pub(super) outcome: CommandAuthorization,
    pub(super) calls: AtomicUsize,
}

#[async_trait]
impl CommandAuthorizer for TestAuthorizer {
    async fn authorize(&self, _: &CommandEnvelope) -> CommandAuthorization {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.outcome
    }
}

pub(super) async fn process(
    store: &TestStore,
    directory: &TestDirectory,
    authorizer: &TestAuthorizer,
) {
    process_command(store, directory, authorizer, &config(), 0, received()).await;
}

pub(super) async fn process_with_deadline(
    store: &TestStore,
    directory: &TestDirectory,
    authorizer: &TestAuthorizer,
    deadline: UnixTimeMillis,
) {
    let mut command = received();
    command.envelope.deadline = deadline;
    process_command(store, directory, authorizer, &config(), 0, command).await;
}

pub(super) fn empty_directory() -> TestDirectory {
    TestDirectory {
        player: None,
        lookups: AtomicUsize::new(0),
    }
}

pub(super) fn authorizer(outcome: CommandAuthorization) -> TestAuthorizer {
    TestAuthorizer {
        outcome,
        calls: AtomicUsize::new(0),
    }
}

fn config() -> CommandWorkerConfig {
    CommandWorkerConfig {
        shards: 0..1,
        group: "test-group".into(),
        consumer: "test-consumer".into(),
        batch_size: 1,
        block: Duration::from_millis(10),
        claim_idle: Duration::from_secs(1),
        lease_ttl: Duration::from_secs(2),
        completion_retention: Duration::from_secs(30),
        retry_delay: Duration::from_millis(10),
    }
}

fn received() -> ReceivedCommand {
    ReceivedCommand {
        entry_id: "1-0".into(),
        envelope: CommandEnvelope {
            command_id: Uuid::from_u128(1),
            idempotency_key: Uuid::from_u128(2),
            guild_id: GuildId::new(10).expect("guild"),
            actor_user_id: Some(UserId::new(20).expect("user")),
            expected_revision: StateRevision::INITIAL,
            deadline: UnixTimeMillis::new(u64::MAX),
            command: PlayerCommand::SetVolume {
                volume: Volume::new(50).expect("volume"),
            },
        },
    }
}
