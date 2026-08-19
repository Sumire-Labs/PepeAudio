use std::{sync::Arc, time::Duration};

use pepeaudio_storage::{CommandConsumer, CommandResultStore, IdempotencyStore};
use tokio::sync::watch;

use crate::{
    CommandAuthorizer, CommandWorkerConfig, PlayerDirectory,
    guild_command_dispatcher::{GuildDispatcher, MAX_CONCURRENT_GUILDS},
};

pub(crate) async fn run_shard<S, D, A>(
    store: S,
    directory: Arc<D>,
    authorizer: Arc<A>,
    config: CommandWorkerConfig,
    shard_id: u32,
    mut shutdown: watch::Receiver<bool>,
) where
    S: CommandConsumer + CommandResultStore + IdempotencyStore + Clone + Send + Sync + 'static,
    D: PlayerDirectory,
    A: CommandAuthorizer,
{
    let buffer_limit = config
        .batch_size
        .saturating_mul(2)
        .max(MAX_CONCURRENT_GUILDS);
    let mut dispatcher = GuildDispatcher::new(
        store.clone(),
        directory,
        authorizer,
        Arc::new(config.clone()),
        shard_id,
        buffer_limit,
    );
    let mut claim_cursor = String::from("0-0");

    loop {
        if *shutdown.borrow() || !dispatcher.reap_finished() {
            return;
        }
        if dispatcher.is_full() {
            if wait_for_capacity(&mut dispatcher, &mut shutdown).await != WaitOutcome::Ready {
                return;
            }
            continue;
        }

        let claim_count = dispatcher.available_capacity().min(config.batch_size);
        let claim = store.claim_abandoned_commands(
            shard_id,
            &config.group,
            &config.consumer,
            config.claim_idle,
            &claim_cursor,
            claim_count,
        );
        let claimed = tokio::select! {
            biased;
            changed = shutdown.changed() => {
                if shutdown_requested(&changed, &shutdown) {
                    return;
                }
                continue;
            }
            result = claim => result,
        };
        match claimed {
            Ok(batch) => {
                claim_cursor = batch.next_start;
                dispatcher.enqueue(batch.commands);
            }
            Err(error) => {
                tracing::warn!(shard_id, %error, "could not claim pending guild commands");
                if wait_for_retry(config.retry_delay, &mut dispatcher, &mut shutdown).await
                    != WaitOutcome::Ready
                {
                    return;
                }
                continue;
            }
        }

        if dispatcher.is_full() {
            continue;
        }
        let read_count = dispatcher.available_capacity().min(config.batch_size);
        let read = store.read_commands(
            shard_id,
            &config.group,
            &config.consumer,
            read_count,
            config.block,
        );
        let received = tokio::select! {
            biased;
            changed = shutdown.changed() => {
                if shutdown_requested(&changed, &shutdown) {
                    return;
                }
                continue;
            }
            result = read => result,
        };
        match received {
            Ok(commands) => dispatcher.enqueue(commands),
            Err(error) => {
                tracing::warn!(shard_id, %error, "could not read guild commands");
                if wait_for_retry(config.retry_delay, &mut dispatcher, &mut shutdown).await
                    != WaitOutcome::Ready
                {
                    return;
                }
            }
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum WaitOutcome {
    Ready,
    Shutdown,
    TaskFailed,
}

async fn wait_for_capacity<S, D, A>(
    dispatcher: &mut GuildDispatcher<S, D, A>,
    shutdown: &mut watch::Receiver<bool>,
) -> WaitOutcome
where
    S: CommandConsumer + CommandResultStore + IdempotencyStore + Clone + Send + Sync + 'static,
    D: PlayerDirectory,
    A: CommandAuthorizer,
{
    while dispatcher.is_full() {
        let outcome = wait_for_completion(dispatcher, shutdown).await;
        if outcome != WaitOutcome::Ready {
            return outcome;
        }
    }
    WaitOutcome::Ready
}

async fn wait_for_retry<S, D, A>(
    delay: Duration,
    dispatcher: &mut GuildDispatcher<S, D, A>,
    shutdown: &mut watch::Receiver<bool>,
) -> WaitOutcome
where
    S: CommandConsumer + CommandResultStore + IdempotencyStore + Clone + Send + Sync + 'static,
    D: PlayerDirectory,
    A: CommandAuthorizer,
{
    let sleep = tokio::time::sleep(delay);
    tokio::pin!(sleep);
    loop {
        tokio::select! {
            biased;
            changed = shutdown.changed() => {
                if shutdown_requested(&changed, shutdown) {
                    return WaitOutcome::Shutdown;
                }
            }
            completed = dispatcher.join_next(), if dispatcher.has_active() => {
                if !dispatcher.finish(completed) {
                    return WaitOutcome::TaskFailed;
                }
            }
            () = &mut sleep => return WaitOutcome::Ready,
        }
    }
}

async fn wait_for_completion<S, D, A>(
    dispatcher: &mut GuildDispatcher<S, D, A>,
    shutdown: &mut watch::Receiver<bool>,
) -> WaitOutcome
where
    S: CommandConsumer + CommandResultStore + IdempotencyStore + Clone + Send + Sync + 'static,
    D: PlayerDirectory,
    A: CommandAuthorizer,
{
    tokio::select! {
        biased;
        changed = shutdown.changed() => {
            if shutdown_requested(&changed, shutdown) {
                WaitOutcome::Shutdown
            } else {
                WaitOutcome::Ready
            }
        }
        completed = dispatcher.join_next(), if dispatcher.has_active() => {
            if dispatcher.finish(completed) {
                WaitOutcome::Ready
            } else {
                WaitOutcome::TaskFailed
            }
        }
    }
}

fn shutdown_requested(
    changed: &Result<(), watch::error::RecvError>,
    shutdown: &watch::Receiver<bool>,
) -> bool {
    changed.is_err() || *shutdown.borrow()
}
