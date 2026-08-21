use std::time::{SystemTime, UNIX_EPOCH};

use pepeaudio_core::{CommandResult, CommandResultCode, UnixTimeMillis};
use pepeaudio_storage::{
    CommandCompletion, CommandConsumer, CommandResultStore, CommandResultWrite, DedupeClaim,
    IdempotencyStore, ReceivedCommand,
};
use uuid::Uuid;

use crate::{
    CommandAuthorization, CommandAuthorizer, CommandExecutionError, CommandWorkerConfig,
    PlayerDirectory, command_outcome,
};

pub(crate) async fn process_command<S, D, A>(
    store: &S,
    directory: &D,
    authorizer: &A,
    config: &CommandWorkerConfig,
    shard_id: u32,
    received: ReceivedCommand,
) where
    S: CommandConsumer + CommandResultStore + IdempotencyStore + Send + Sync,
    D: PlayerDirectory,
    A: CommandAuthorizer,
{
    let guild_id = received.envelope.guild_id;
    let command_id = received.envelope.command_id;
    match store.get_command_result(guild_id, command_id).await {
        Ok(Some(result)) if result.is_terminal() => {
            acknowledge(store, config, shard_id, &received.entry_id).await;
            return;
        }
        Ok(Some(_pending)) => {}
        Ok(None) => {
            let expired = CommandResult::rejected(
                command_id,
                guild_id,
                CommandResultCode::ResultExpired,
                None,
            );
            store_result_and_acknowledge(store, config, shard_id, &received.entry_id, &expired)
                .await;
            return;
        }
        Err(_) => return,
    }

    if deadline_expired(received.envelope.deadline) {
        let expired = CommandResult::rejected(
            command_id,
            guild_id,
            CommandResultCode::DeadlineExpired,
            None,
        );
        store_result_and_acknowledge(store, config, shard_id, &received.entry_id, &expired).await;
        return;
    }

    match authorizer.authorize(&received.envelope).await {
        CommandAuthorization::Allowed => {}
        CommandAuthorization::Denied => {
            let denied =
                CommandResult::denied(command_id, guild_id, CommandResultCode::NotAuthorized);
            store_result_and_acknowledge(store, config, shard_id, &received.entry_id, &denied)
                .await;
            return;
        }
        CommandAuthorization::RetryableFailure => return,
    }

    let idempotency_key = received.envelope.idempotency_key;
    let lease_token = Uuid::new_v4();
    let claim = store
        .claim_idempotency(guild_id, idempotency_key, lease_token, config.lease_ttl)
        .await;
    match claim {
        Ok(DedupeClaim::Completed) => {
            let replayed = CommandResult::rejected(
                command_id,
                guild_id,
                CommandResultCode::IdempotencyReplayed,
                None,
            );
            store_result_and_acknowledge(store, config, shard_id, &received.entry_id, &replayed)
                .await;
        }
        Ok(DedupeClaim::InProgress) | Err(_) => {}
        Ok(DedupeClaim::Acquired) => {
            let outcome = directory.execute(received.envelope).await;
            let terminal = match outcome {
                Ok(snapshot) => Some(command_outcome::applied(
                    command_id,
                    guild_id,
                    snapshot.guild_id,
                    snapshot.revision,
                )),
                Err(CommandExecutionError::Player(error)) => {
                    command_outcome::rejected(command_id, guild_id, &error)
                }
                Err(CommandExecutionError::Rejected(code)) => {
                    Some(CommandResult::rejected(command_id, guild_id, code, None))
                }
                Err(CommandExecutionError::Retryable) => None,
            };
            if let Some(result) = terminal {
                complete_with_result_and_acknowledge(
                    store,
                    config,
                    shard_id,
                    &received.entry_id,
                    idempotency_key,
                    lease_token,
                    &result,
                )
                .await;
            } else {
                let _released = store
                    .release_idempotency(guild_id, idempotency_key, lease_token)
                    .await;
            }
        }
    }
}

fn deadline_expired(deadline: UnixTimeMillis) -> bool {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    u64::try_from(now).unwrap_or(u64::MAX) >= deadline.get()
}

#[allow(clippy::too_many_arguments)]
async fn complete_with_result_and_acknowledge<S>(
    store: &S,
    config: &CommandWorkerConfig,
    shard_id: u32,
    entry_id: &str,
    idempotency_key: Uuid,
    lease_token: Uuid,
    result: &CommandResult,
) where
    S: CommandConsumer + CommandResultStore + IdempotencyStore + Send + Sync,
{
    let completed = store
        .complete_command(
            idempotency_key,
            lease_token,
            result,
            config.completion_retention,
            config.completion_retention,
        )
        .await;
    if matches!(
        completed,
        Ok(CommandCompletion::Stored | CommandCompletion::Unchanged)
    ) {
        acknowledge(store, config, shard_id, entry_id).await;
    }
}

async fn store_result_and_acknowledge<S>(
    store: &S,
    config: &CommandWorkerConfig,
    shard_id: u32,
    entry_id: &str,
    result: &CommandResult,
) where
    S: CommandConsumer + CommandResultStore + Send + Sync,
{
    let stored = store
        .store_command_result(result, config.completion_retention)
        .await;
    if matches!(
        stored,
        Ok(CommandResultWrite::Stored | CommandResultWrite::Unchanged)
    ) {
        acknowledge(store, config, shard_id, entry_id).await;
    }
}

async fn acknowledge<S>(store: &S, config: &CommandWorkerConfig, shard_id: u32, entry_id: &str)
where
    S: CommandConsumer + Send + Sync,
{
    let _acknowledged = store
        .acknowledge_command(shard_id, &config.group, entry_id)
        .await;
}
