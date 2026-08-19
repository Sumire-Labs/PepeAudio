use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use pepeaudio_core::{
    GuildId, HrirPresetId, PlayerSnapshot, PlayerState, RepeatMode, StateRevision, UnixTimeMillis,
    Volume,
};
use pepeaudio_player::{NoopSnapshotPublisher, SnapshotPublisher};
use pepeaudio_storage::{
    ControlPolicy, GuildSettings, GuildSettingsRepository, SettingsRevision, StorageError,
    StorageResult,
};
use time::OffsetDateTime;

use crate::{PersistentPlayerSettings, PersistentSnapshotPublisher, SettingsPersistenceRuntime};

#[derive(Clone)]
struct FakeRepository {
    state: Arc<Mutex<FakeState>>,
}

struct FakeState {
    row: GuildSettings,
    updates: usize,
    failures_remaining: usize,
    conflict_once: bool,
    panic_on_update: bool,
}

impl FakeRepository {
    fn new(row: GuildSettings) -> Self {
        Self {
            state: Arc::new(Mutex::new(FakeState {
                row,
                updates: 0,
                failures_remaining: 0,
                conflict_once: false,
                panic_on_update: false,
            })),
        }
    }

    fn row(&self) -> GuildSettings {
        self.state.lock().expect("fake lock").row.clone()
    }

    fn updates(&self) -> usize {
        self.state.lock().expect("fake lock").updates
    }

    fn force_conflict(&self) {
        self.state.lock().expect("fake lock").conflict_once = true;
    }

    fn fail_updates(&self, count: usize) {
        self.state.lock().expect("fake lock").failures_remaining = count;
    }

    fn panic_updates(&self) {
        self.state.lock().expect("fake lock").panic_on_update = true;
    }
}

#[async_trait]
impl GuildSettingsRepository for FakeRepository {
    async fn get_guild_settings(&self, guild_id: GuildId) -> StorageResult<Option<GuildSettings>> {
        let state = self.state.lock().expect("fake lock");
        Ok((state.row.guild_id == guild_id).then(|| state.row.clone()))
    }

    async fn create_guild_settings(
        &self,
        defaults: &GuildSettings,
    ) -> StorageResult<GuildSettings> {
        Ok(defaults.clone())
    }

    async fn update_guild_settings(
        &self,
        settings: &GuildSettings,
        expected_revision: SettingsRevision,
    ) -> StorageResult<Option<GuildSettings>> {
        let mut state = self.state.lock().expect("fake lock");
        assert!(!state.panic_on_update, "intentional settings worker panic");
        if state.failures_remaining > 0 {
            state.failures_remaining -= 1;
            return Err(StorageError::CapacityExceeded {
                resource: "test settings writer",
            });
        }
        if state.conflict_once {
            state.conflict_once = false;
            state.row.control_policy = ControlPolicy::ManageGuild;
            state.row.dj_role_id = Some(99);
            state.row.revision = SettingsRevision::new(state.row.revision.get() + 1);
            return Ok(None);
        }
        if state.row.revision != expected_revision {
            return Ok(None);
        }
        let mut updated = settings.clone();
        updated.revision = SettingsRevision::new(expected_revision.get() + 1);
        state.row = updated.clone();
        state.updates += 1;
        Ok(Some(updated))
    }
}

#[tokio::test]
async fn initialization_intermediates_are_not_persisted() {
    let repository = FakeRepository::new(seed_settings(true));
    let runtime = SettingsPersistenceRuntime::start(repository.clone());
    let handle = runtime.handle();
    let initial =
        PersistentPlayerSettings::from_guild_settings(&repository.row()).expect("seed HRIR");
    let settings = handle
        .publisher(guild(), repository.row(), initial)
        .await
        .expect("publisher");
    let mut publisher = PersistentSnapshotPublisher::new(NoopSnapshotPublisher, settings);

    publisher
        .publish(&snapshot(1, None, 75, false))
        .await
        .expect("missing HRIR is an initialization intermediate");
    publisher
        .publish(&snapshot(2, Some("alpha"), 75, false))
        .await
        .expect("spatial initialization intermediate");
    publisher
        .publish(&snapshot(3, Some("alpha"), 75, true))
        .await
        .expect("final initial state arms persistence");
    tokio::task::yield_now().await;
    assert_eq!(repository.updates(), 0);

    publisher
        .publish(&snapshot(4, Some("alpha"), 60, true))
        .await
        .expect("changed setting accepted");
    wait_for_updates(&repository, 1).await;
    assert_eq!(repository.row().volume.percent(), 60);
    let latest = handle.latest(guild()).await.expect("registered worker");
    assert!(latest.pending.is_none());
    assert_eq!(latest.durable.volume.percent(), 60);
    runtime.shutdown().await.expect("clean shutdown");
}

#[tokio::test]
async fn optimistic_conflict_reloads_and_preserves_policy_fields() {
    let repository = FakeRepository::new(seed_settings(false));
    repository.force_conflict();
    let runtime = SettingsPersistenceRuntime::start(repository.clone());
    let initial =
        PersistentPlayerSettings::from_guild_settings(&repository.row()).expect("seed HRIR");
    let settings = runtime
        .handle()
        .publisher(guild(), repository.row(), initial)
        .await
        .expect("publisher");
    let mut publisher = PersistentSnapshotPublisher::new(NoopSnapshotPublisher, settings);
    publisher
        .publish(&snapshot(1, Some("alpha"), 75, false))
        .await
        .expect("arm");
    publisher
        .publish(&snapshot(2, Some("beta"), 40, true))
        .await
        .expect("queue desired settings");

    wait_for_updates(&repository, 1).await;
    let row = repository.row();
    assert_eq!(row.control_policy, ControlPolicy::ManageGuild);
    assert_eq!(row.dj_role_id, Some(99));
    assert_eq!(row.volume.percent(), 40);
    assert_eq!(
        row.default_hrir_preset_id
            .as_ref()
            .map(HrirPresetId::as_str),
        Some("beta")
    );
    assert!(row.spatial_audio_enabled);
    runtime.shutdown().await.expect("clean shutdown");
}

#[tokio::test(start_paused = true)]
async fn transient_failure_retries_only_the_latest_accepted_defaults() {
    let repository = FakeRepository::new(seed_settings(false));
    repository.fail_updates(1);
    let runtime = SettingsPersistenceRuntime::start(repository.clone());
    let initial =
        PersistentPlayerSettings::from_guild_settings(&repository.row()).expect("seed HRIR");
    let settings = runtime
        .handle()
        .publisher(guild(), repository.row(), initial)
        .await
        .expect("publisher");
    let mut publisher = PersistentSnapshotPublisher::new(NoopSnapshotPublisher, settings);
    publisher
        .publish(&snapshot(1, Some("alpha"), 75, false))
        .await
        .expect("arm");
    publisher
        .publish(&snapshot(2, Some("beta"), 60, true))
        .await
        .expect("first change");
    tokio::task::yield_now().await;
    publisher
        .publish(&snapshot(3, Some("gamma"), 35, false))
        .await
        .expect("newest change replaces retry payload");

    tokio::time::advance(std::time::Duration::from_millis(200)).await;
    wait_for_updates(&repository, 1).await;
    let row = repository.row();
    assert_eq!(row.volume.percent(), 35);
    assert_eq!(
        row.default_hrir_preset_id
            .as_ref()
            .map(HrirPresetId::as_str),
        Some("gamma")
    );
    assert!(!row.spatial_audio_enabled);
    assert_eq!(repository.updates(), 1);
    runtime.shutdown().await.expect("clean shutdown");
}

#[tokio::test]
async fn latest_view_never_regresses_after_a_write_clears_pending() {
    let repository = FakeRepository::new(seed_settings(false));
    let runtime = SettingsPersistenceRuntime::start(repository.clone());
    let handle = runtime.handle();
    let initial =
        PersistentPlayerSettings::from_guild_settings(&repository.row()).expect("seed HRIR");
    let settings = handle
        .publisher(guild(), repository.row(), initial)
        .await
        .expect("publisher");
    let mut publisher = PersistentSnapshotPublisher::new(NoopSnapshotPublisher, settings);
    publisher
        .publish(&snapshot(1, Some("alpha"), 75, false))
        .await
        .expect("arm");
    publisher
        .publish(&snapshot(2, Some("beta"), 45, true))
        .await
        .expect("persist update");

    wait_for_updates(&repository, 1).await;
    let view = handle.latest(guild()).await.expect("registered worker");
    assert!(view.pending.is_none());
    assert_eq!(view.durable.volume.percent(), 45);
    assert_eq!(
        view.durable
            .default_hrir_preset_id
            .as_ref()
            .map(HrirPresetId::as_str),
        Some("beta")
    );
    assert!(view.durable.spatial_audio_enabled);
    runtime.shutdown().await.expect("clean shutdown");
}

#[tokio::test]
async fn worker_panic_is_reported_before_process_shutdown() {
    let repository = FakeRepository::new(seed_settings(false));
    repository.panic_updates();
    let mut runtime = SettingsPersistenceRuntime::start(repository.clone());
    let initial =
        PersistentPlayerSettings::from_guild_settings(&repository.row()).expect("seed HRIR");
    let settings = runtime
        .handle()
        .publisher(guild(), repository.row(), initial)
        .await
        .expect("publisher");
    let mut publisher = PersistentSnapshotPublisher::new(NoopSnapshotPublisher, settings);
    publisher
        .publish(&snapshot(1, Some("alpha"), 75, false))
        .await
        .expect("arm");
    publisher
        .publish(&snapshot(2, Some("beta"), 50, true))
        .await
        .expect("queue update");

    let error = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        runtime.wait_for_unexpected_exit(),
    )
    .await
    .expect("failure monitor wakes");
    assert!(matches!(
        error,
        crate::SettingsSupervisorError::WorkerPanicked { guild_id } if guild_id == guild()
    ));
    assert!(runtime.shutdown().await.is_err());
}

async fn wait_for_updates(repository: &FakeRepository, expected: usize) {
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while repository.updates() < expected {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("settings worker made progress");
}

fn seed_settings(spatial_audio_enabled: bool) -> GuildSettings {
    GuildSettings {
        guild_id: guild(),
        volume: Volume::new(75).expect("volume"),
        idle_disconnect: std::time::Duration::from_mins(5),
        control_policy: ControlPolicy::SameVoiceChannel,
        dj_role_id: None,
        default_hrir_preset_id: Some(HrirPresetId::new("alpha").expect("HRIR")),
        spatial_audio_enabled,
        revision: SettingsRevision::INITIAL,
        created_at: OffsetDateTime::UNIX_EPOCH,
        updated_at: OffsetDateTime::UNIX_EPOCH,
    }
}

fn snapshot(
    revision: u64,
    hrir: Option<&str>,
    volume: u8,
    spatial_audio_enabled: bool,
) -> PlayerSnapshot {
    PlayerSnapshot {
        guild_id: guild(),
        voice_channel_id: None,
        revision: StateRevision::new(revision),
        state: PlayerState::Disconnected,
        current_track: None,
        queued_tracks: 0,
        upcoming_tracks: Vec::new(),
        has_previous_track: false,
        volume: Volume::new(volume).expect("volume"),
        repeat_mode: RepeatMode::Off,
        shuffle_enabled: false,
        hrir_preset: hrir.map(|value| HrirPresetId::new(value).expect("HRIR")),
        spatial_audio_enabled,
        observed_at: UnixTimeMillis::new(0),
    }
}

fn guild() -> GuildId {
    GuildId::new(42).expect("guild")
}
