use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use pepeaudio_core::GuildId;
use pepeaudio_runtime::GuildPresenceHandle;
use tokio::{sync::mpsc, task::JoinSet, time::Instant};

use crate::guild_lifecycle::{
    GuildAbsence, GuildLifecycleError, LifecycleCommand, RetryPolicy, ShardReconciliation,
    SnapshotInvalidator,
};

const MAX_PARALLEL_INVALIDATIONS: usize = 32;

type OwnedGuilds = HashMap<u32, HashSet<GuildId>>;

#[derive(Clone, Copy, Eq, PartialEq)]
enum AdvertisementStage {
    NeedsInvalidation,
    NeedsPresence,
    Advertised,
}

#[derive(Default)]
struct LifecycleState {
    owned: OwnedGuilds,
    advertisements: HashMap<GuildId, AdvertisementStage>,
    pending_absence: HashSet<GuildId>,
}

impl LifecycleState {
    fn reconcile_shard(
        &mut self,
        shard_id: u32,
        guilds: &HashSet<GuildId>,
    ) -> (HashSet<GuildId>, Vec<GuildId>) {
        let previous = if guilds.is_empty() {
            self.owned.remove(&shard_id).unwrap_or_default()
        } else {
            self.owned
                .insert(shard_id, guilds.clone())
                .unwrap_or_default()
        };
        let affected: HashSet<_> = previous.union(guilds).copied().collect();
        let removed = self.apply_desired_state(&affected);
        (affected, removed)
    }

    fn present(&mut self, shard_id: u32, guild_id: GuildId) -> HashSet<GuildId> {
        self.owned.entry(shard_id).or_default().insert(guild_id);
        let affected = HashSet::from([guild_id]);
        self.apply_desired_state(&affected);
        affected
    }

    fn absent(&mut self, shard_id: u32, guild_id: GuildId) -> (HashSet<GuildId>, bool) {
        if let Some(guilds) = self.owned.get_mut(&shard_id) {
            guilds.remove(&guild_id);
            if guilds.is_empty() {
                self.owned.remove(&shard_id);
            }
        }
        let affected = HashSet::from([guild_id]);
        let removed = self.apply_desired_state(&affected);
        (affected, removed.contains(&guild_id))
    }

    fn apply_desired_state(&mut self, affected: &HashSet<GuildId>) -> Vec<GuildId> {
        let mut removed = Vec::new();
        for guild_id in affected.iter().copied() {
            if self.is_desired(guild_id) {
                self.pending_absence.remove(&guild_id);
                self.advertisements
                    .entry(guild_id)
                    .or_insert(AdvertisementStage::NeedsInvalidation);
            } else {
                if self.advertisements.remove(&guild_id).is_some() {
                    removed.push(guild_id);
                }
                self.pending_absence.insert(guild_id);
            }
        }
        removed
    }

    fn is_desired(&self, guild_id: GuildId) -> bool {
        self.owned.values().any(|guilds| guilds.contains(&guild_id))
    }

    fn pending(&self) -> HashSet<GuildId> {
        self.advertisements
            .iter()
            .filter_map(|(guild_id, stage)| {
                (*stage != AdvertisementStage::Advertised).then_some(*guild_id)
            })
            .chain(self.pending_absence.iter().copied())
            .collect()
    }

    fn owned_on_shard(&self, shard_id: u32) -> Vec<GuildId> {
        self.owned
            .get(&shard_id)
            .map_or_else(Vec::new, |guilds| guilds.iter().copied().collect())
    }
}

pub(super) async fn run_actor(
    snapshots: Arc<dyn SnapshotInvalidator>,
    presence: GuildPresenceHandle,
    retry: RetryPolicy,
    mut receiver: mpsc::Receiver<LifecycleCommand>,
) {
    let mut state = LifecycleState::default();
    let start = Instant::now() + retry.background_interval;
    let mut interval = tokio::time::interval_at(start, retry.background_interval);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            biased;
            command = receiver.recv() => {
                let Some(command) = command else { return };
                match command {
                    LifecycleCommand::ReconcileShard { shard_id, guilds, reply } => {
                        let (affected, removed) = state.reconcile_shard(shard_id, &guilds);
                        let update = reconcile_with_retries(
                            &mut state, &snapshots, &presence, affected, retry.short_delays,
                        ).await;
                        let _ = reply.send(ShardReconciliation { removed, update });
                    }
                    LifecycleCommand::Present { shard_id, guild_id, reply } => {
                        let affected = state.present(shard_id, guild_id);
                        let result = reconcile_with_retries(
                            &mut state, &snapshots, &presence, affected, retry.short_delays,
                        ).await;
                        let _ = reply.send(result);
                    }
                    LifecycleCommand::Absent { shard_id, guild_id, reply } => {
                        let (affected, no_longer_owned) = state.absent(shard_id, guild_id);
                        let update = reconcile_with_retries(
                            &mut state, &snapshots, &presence, affected, retry.short_delays,
                        ).await;
                        let _ = reply.send(GuildAbsence { no_longer_owned, update });
                    }
                    LifecycleCommand::Owned { shard_id, reply } => {
                        let _ = reply.send(state.owned_on_shard(shard_id));
                    }
                    LifecycleCommand::Shutdown(reply) => {
                        let _ = reply.send(());
                        return;
                    }
                }
            }
            _ = interval.tick() => {
                let pending = state.pending();
                if !pending.is_empty()
                    && reconcile_once(&mut state, &snapshots, &presence, pending).await.is_err()
                {
                    tracing::warn!("guild lifecycle reconciliation remains pending");
                }
            }
        }
    }
}

async fn reconcile_with_retries(
    state: &mut LifecycleState,
    snapshots: &Arc<dyn SnapshotInvalidator>,
    presence: &GuildPresenceHandle,
    affected: HashSet<GuildId>,
    delays: [std::time::Duration; 2],
) -> Result<(), GuildLifecycleError> {
    let mut pending = reconcile_round(state, snapshots, presence, affected).await;
    for delay in delays {
        if pending.is_empty() {
            return Ok(());
        }
        tokio::time::sleep(delay).await;
        pending = reconcile_round(state, snapshots, presence, pending).await;
    }
    pending.is_empty().then_some(()).ok_or(GuildLifecycleError)
}

async fn reconcile_once(
    state: &mut LifecycleState,
    snapshots: &Arc<dyn SnapshotInvalidator>,
    presence: &GuildPresenceHandle,
    affected: HashSet<GuildId>,
) -> Result<(), GuildLifecycleError> {
    reconcile_round(state, snapshots, presence, affected)
        .await
        .is_empty()
        .then_some(())
        .ok_or(GuildLifecycleError)
}

async fn reconcile_round(
    state: &mut LifecycleState,
    snapshots: &Arc<dyn SnapshotInvalidator>,
    presence: &GuildPresenceHandle,
    affected: HashSet<GuildId>,
) -> HashSet<GuildId> {
    let invalidations: Vec<_> = affected
        .iter()
        .copied()
        .filter(|guild_id| {
            state.advertisements.get(guild_id) == Some(&AdvertisementStage::NeedsInvalidation)
        })
        .collect();
    for guild_id in invalidate_all(snapshots, &invalidations).await {
        state
            .advertisements
            .insert(guild_id, AdvertisementStage::NeedsPresence);
    }

    let advertisements: Vec<_> = affected
        .iter()
        .copied()
        .filter(|guild_id| {
            state.advertisements.get(guild_id) == Some(&AdvertisementStage::NeedsPresence)
        })
        .collect();
    for guild_id in advertisements {
        if presence.present(guild_id).await.is_ok() {
            state
                .advertisements
                .insert(guild_id, AdvertisementStage::Advertised);
        }
    }

    let absences: Vec<_> = affected
        .iter()
        .copied()
        .filter(|guild_id| state.pending_absence.contains(guild_id))
        .collect();
    for guild_id in absences {
        if presence.absent(guild_id).await.is_ok() {
            state.pending_absence.remove(&guild_id);
        }
    }

    affected
        .into_iter()
        .filter(|guild_id| {
            if state.is_desired(*guild_id) {
                state.advertisements.get(guild_id) != Some(&AdvertisementStage::Advertised)
            } else {
                state.pending_absence.contains(guild_id)
            }
        })
        .collect()
}

async fn invalidate_all(
    snapshots: &Arc<dyn SnapshotInvalidator>,
    guilds: &[GuildId],
) -> Vec<GuildId> {
    let mut succeeded = Vec::new();
    for chunk in guilds.chunks(MAX_PARALLEL_INVALIDATIONS) {
        let mut tasks = JoinSet::new();
        for guild_id in chunk.iter().copied() {
            let snapshots = Arc::clone(snapshots);
            tasks.spawn(async move { (guild_id, snapshots.invalidate(guild_id).await) });
        }
        while let Some(result) = tasks.join_next().await {
            match result {
                Ok((guild_id, Ok(()))) => succeeded.push(guild_id),
                Ok((_guild_id, Err(_))) => {}
                Err(error) => {
                    tracing::warn!(error = %error, "snapshot invalidation task failed");
                }
            }
        }
    }
    succeeded
}
