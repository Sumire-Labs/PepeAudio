use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::Arc,
};

use pepeaudio_core::GuildId;
use pepeaudio_storage::{CommandConsumer, CommandResultStore, IdempotencyStore, ReceivedCommand};
use tokio::task::{JoinError, JoinSet};

use crate::{
    CommandAuthorizer, CommandWorkerConfig, PlayerDirectory, command_loop::process_command,
};

pub(crate) const MAX_CONCURRENT_GUILDS: usize = 32;

pub(crate) struct GuildDispatcher<S, D, A> {
    store: S,
    directory: Arc<D>,
    authorizer: Arc<A>,
    config: Arc<CommandWorkerConfig>,
    shard_id: u32,
    buffer_limit: usize,
    buffered: usize,
    queues: HashMap<GuildId, VecDeque<ReceivedCommand>>,
    ready: VecDeque<GuildId>,
    active: HashSet<GuildId>,
    tasks: JoinSet<(GuildId, usize)>,
}

impl<S, D, A> GuildDispatcher<S, D, A>
where
    S: CommandConsumer + CommandResultStore + IdempotencyStore + Clone + Send + Sync + 'static,
    D: PlayerDirectory,
    A: CommandAuthorizer,
{
    pub(crate) fn new(
        store: S,
        directory: Arc<D>,
        authorizer: Arc<A>,
        config: Arc<CommandWorkerConfig>,
        shard_id: u32,
        buffer_limit: usize,
    ) -> Self {
        Self {
            store,
            directory,
            authorizer,
            config,
            shard_id,
            buffer_limit,
            buffered: 0,
            queues: HashMap::new(),
            ready: VecDeque::new(),
            active: HashSet::new(),
            tasks: JoinSet::new(),
        }
    }

    pub(crate) fn enqueue(&mut self, commands: Vec<ReceivedCommand>) {
        for command in commands {
            let guild_id = command.envelope.guild_id;
            let queue = self.queues.entry(guild_id).or_default();
            let needs_ready_entry = queue.is_empty() && !self.active.contains(&guild_id);
            queue.push_back(command);
            self.buffered = self.buffered.saturating_add(1);
            if needs_ready_entry {
                self.ready.push_back(guild_id);
            }
        }
        self.spawn_ready();
    }

    fn spawn_ready(&mut self) {
        while self.tasks.len() < MAX_CONCURRENT_GUILDS {
            let Some(guild_id) = self.ready.pop_front() else {
                break;
            };
            if self.active.contains(&guild_id) {
                continue;
            }
            let Some(queue) = self.queues.get_mut(&guild_id) else {
                continue;
            };
            let commands = queue.drain(..).collect::<Vec<_>>();
            if commands.is_empty() || !self.active.insert(guild_id) {
                continue;
            }

            let store = self.store.clone();
            let directory = Arc::clone(&self.directory);
            let authorizer = Arc::clone(&self.authorizer);
            let config = Arc::clone(&self.config);
            let shard_id = self.shard_id;
            self.tasks.spawn(async move {
                let processed = commands.len();
                for command in commands {
                    process_command(
                        &store,
                        directory.as_ref(),
                        authorizer.as_ref(),
                        config.as_ref(),
                        shard_id,
                        command,
                    )
                    .await;
                }
                (guild_id, processed)
            });
        }
    }

    pub(crate) fn reap_finished(&mut self) -> bool {
        while let Some(completed) = self.tasks.try_join_next() {
            if !self.finish(Some(completed)) {
                return false;
            }
        }
        true
    }

    pub(crate) async fn join_next(&mut self) -> Option<Result<(GuildId, usize), JoinError>> {
        self.tasks.join_next().await
    }

    pub(crate) fn finish(
        &mut self,
        completed: Option<Result<(GuildId, usize), JoinError>>,
    ) -> bool {
        let Some(completed) = completed else {
            tracing::error!(
                shard_id = self.shard_id,
                "guild command dispatcher stopped with work remaining"
            );
            return false;
        };
        let (guild_id, processed) = match completed {
            Ok(completed) => completed,
            Err(error) => {
                tracing::error!(
                    shard_id = self.shard_id,
                    %error,
                    "guild command task failed"
                );
                return false;
            }
        };
        self.active.remove(&guild_id);
        self.buffered = self.buffered.saturating_sub(processed);
        if self
            .queues
            .get(&guild_id)
            .is_some_and(|queue| !queue.is_empty())
        {
            self.ready.push_back(guild_id);
        } else {
            self.queues.remove(&guild_id);
        }
        self.spawn_ready();
        true
    }

    pub(crate) fn has_active(&self) -> bool {
        !self.tasks.is_empty()
    }

    pub(crate) fn is_full(&self) -> bool {
        self.buffered >= self.buffer_limit
    }

    pub(crate) fn available_capacity(&self) -> usize {
        self.buffer_limit.saturating_sub(self.buffered)
    }
}
