use std::{
    collections::HashMap,
    future,
    sync::{Arc, Mutex},
    time::Duration,
};

use async_trait::async_trait;
use pepeaudio_core::{
    GuildId, PlayerSnapshot, PlayerState, RepeatMode, StateRevision, UnixTimeMillis, Volume,
};
use pepeaudio_storage::{SnapshotStore, SnapshotWrite, StorageError, StorageResult};
use tokio::sync::Notify;

use crate::{SnapshotPublisherRuntime, snapshot_worker::SnapshotWorkerSettings};

pub(super) const RETRY: Duration = Duration::from_millis(10);
pub(super) const WRITE_TIMEOUT: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, Default)]
pub(super) enum Behavior {
    #[default]
    Succeed,
    Fail,
    Hang,
}

#[derive(Clone, Default)]
pub(super) struct FakeStore {
    state: Arc<Mutex<FakeState>>,
    changed: Arc<Notify>,
}

#[derive(Default)]
struct FakeState {
    behavior: HashMap<GuildId, Behavior>,
    attempts: Vec<(GuildId, StateRevision)>,
    stored: Vec<(GuildId, StateRevision)>,
}

impl FakeStore {
    pub(super) fn set_behavior(&self, guild_id: GuildId, behavior: Behavior) {
        self.state
            .lock()
            .expect("fake store lock")
            .behavior
            .insert(guild_id, behavior);
    }

    pub(super) fn attempts(&self, guild_id: GuildId) -> Vec<StateRevision> {
        self.state
            .lock()
            .expect("fake store lock")
            .attempts
            .iter()
            .filter_map(|(guild, revision)| (*guild == guild_id).then_some(*revision))
            .collect()
    }

    pub(super) fn stored(&self, guild_id: GuildId) -> Vec<StateRevision> {
        self.state
            .lock()
            .expect("fake store lock")
            .stored
            .iter()
            .filter_map(|(guild, revision)| (*guild == guild_id).then_some(*revision))
            .collect()
    }

    pub(super) async fn wait_for_attempts(&self, guild_id: GuildId, count: usize) {
        loop {
            let notified = self.changed.notified();
            if self.attempts(guild_id).len() >= count {
                return;
            }
            notified.await;
        }
    }

    pub(super) async fn wait_for_stored(&self, guild_id: GuildId, count: usize) {
        loop {
            let notified = self.changed.notified();
            if self.stored(guild_id).len() >= count {
                return;
            }
            notified.await;
        }
    }
}

#[async_trait]
impl SnapshotStore for FakeStore {
    async fn get_snapshot(&self, _: GuildId) -> StorageResult<Option<PlayerSnapshot>> {
        Ok(None)
    }

    async fn get_snapshot_revision(
        &self,
        guild_id: GuildId,
    ) -> StorageResult<Option<StateRevision>> {
        Ok(self.stored(guild_id).into_iter().max())
    }

    async fn invalidate_snapshot(&self, _: GuildId) -> StorageResult<()> {
        Ok(())
    }

    async fn put_snapshot_if_newer(
        &self,
        snapshot: &PlayerSnapshot,
        _: Duration,
    ) -> StorageResult<SnapshotWrite> {
        let behavior = {
            let mut state = self.state.lock().expect("fake store lock");
            state.attempts.push((snapshot.guild_id, snapshot.revision));
            *state
                .behavior
                .get(&snapshot.guild_id)
                .unwrap_or(&Behavior::Succeed)
        };
        self.changed.notify_one();
        match behavior {
            Behavior::Succeed => {
                self.state
                    .lock()
                    .expect("fake store lock")
                    .stored
                    .push((snapshot.guild_id, snapshot.revision));
                self.changed.notify_one();
                Ok(SnapshotWrite::Stored)
            }
            Behavior::Fail => Err(StorageError::InvalidIdentifier {
                kind: "fake_snapshot",
                reason: "injected failure",
            }),
            Behavior::Hang => future::pending().await,
        }
    }
}

pub(super) fn runtime(store: FakeStore) -> SnapshotPublisherRuntime<FakeStore> {
    SnapshotPublisherRuntime::start_with_settings(
        store,
        Duration::from_mins(1),
        SnapshotWorkerSettings::new(RETRY, Duration::from_millis(40), WRITE_TIMEOUT),
    )
    .expect("snapshot runtime")
}

pub(super) fn snapshot(guild_id: GuildId, revision: u64) -> PlayerSnapshot {
    PlayerSnapshot {
        guild_id,
        voice_channel_id: None,
        revision: StateRevision::new(revision),
        state: PlayerState::Disconnected,
        current_track: None,
        queued_tracks: 0,
        upcoming_tracks: Vec::new(),
        has_previous_track: false,
        volume: Volume::DEFAULT,
        repeat_mode: RepeatMode::Off,
        shuffle_enabled: false,
        hrir_preset: None,
        spatial_audio_enabled: false,
        observed_at: UnixTimeMillis::new(revision),
    }
}

pub(super) fn guild(value: u64) -> GuildId {
    GuildId::new(value).expect("guild ID")
}
