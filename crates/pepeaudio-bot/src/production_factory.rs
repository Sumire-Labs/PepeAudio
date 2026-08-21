use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use pepeaudio_core::{CommandEnvelope, GuildId, PlayerCommand, StateRevision, UnixTimeMillis};
use pepeaudio_pipeline::{
    FfmpegDecoderFactory, HrirProvider, PipelineConfig, PipelineDependencies, SongbirdPlayback,
    TrackResolver,
};
use pepeaudio_player::{PlayerConfig, PlayerHandle, spawn_player_with_revision};
use pepeaudio_presets::HrirCatalog;
use pepeaudio_runtime::{
    PersistentPlayerSettings, PersistentSnapshotPublisher, SettingsPersistenceHandle,
    SnapshotPublisherHandle, WorkerPlayerError,
};
use pepeaudio_storage::{
    ControlPolicy as StoredControlPolicy, GuildSettings, GuildSettingsRepository, PostgresStorage,
    SettingsRevision, SnapshotStore, ValkeyStore,
};
use songbird::Songbird;

use crate::{PlayerFactory, PlayerRegistry, RegistryError};

pub(crate) struct ProductionPlayerFactory {
    manager: Arc<Songbird>,
    resolver: Arc<dyn TrackResolver>,
    decoder: Arc<FfmpegDecoderFactory>,
    hrirs: Arc<dyn HrirProvider>,
    catalog: HrirCatalog,
    postgres: PostgresStorage,
    valkey: ValkeyStore,
    snapshots: SnapshotPublisherHandle<ValkeyStore>,
    settings_persistence: SettingsPersistenceHandle<PostgresStorage>,
    defaults: pepeaudio_config::PlayerLimits,
}

impl ProductionPlayerFactory {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        manager: Arc<Songbird>,
        resolver: Arc<dyn TrackResolver>,
        decoder: Arc<FfmpegDecoderFactory>,
        hrirs: Arc<dyn HrirProvider>,
        catalog: HrirCatalog,
        postgres: PostgresStorage,
        valkey: ValkeyStore,
        snapshots: SnapshotPublisherHandle<ValkeyStore>,
        settings_persistence: SettingsPersistenceHandle<PostgresStorage>,
        defaults: pepeaudio_config::PlayerLimits,
    ) -> Result<Self, RegistryError> {
        if catalog.is_empty() {
            return Err(RegistryError::Factory(
                "at least one validated HeSuVi HRIR preset must be installed".into(),
            ));
        }
        Ok(Self {
            manager,
            resolver,
            decoder,
            hrirs,
            catalog,
            postgres,
            valkey,
            snapshots,
            settings_persistence,
            defaults,
        })
    }

    async fn settings(&self, guild_id: GuildId) -> Result<InitialSettings, RegistryError> {
        let stored = self
            .postgres
            .get_guild_settings(guild_id)
            .await
            .map_err(|_| RegistryError::Factory("guild settings are unavailable".into()))?;
        let default_preset = self.defaults.default_hrir_preset.clone();
        if self.catalog.get(&default_preset).is_none() {
            return Err(RegistryError::Factory(
                "configured default HRIR preset is unavailable".into(),
            ));
        }
        let stored = if let Some(stored) = stored {
            stored
        } else {
            let now = time::OffsetDateTime::now_utc();
            let defaults = GuildSettings {
                guild_id,
                volume: self.defaults.default_volume,
                idle_disconnect: self.defaults.idle_disconnect,
                control_policy: StoredControlPolicy::SameVoiceChannel,
                dj_role_id: None,
                default_hrir_preset_id: Some(default_preset.clone()),
                spatial_audio_enabled: self.defaults.default_spatial_audio_enabled,
                revision: SettingsRevision::INITIAL,
                created_at: now,
                updated_at: now,
            };
            self.postgres
                .create_guild_settings(&defaults)
                .await
                .map_err(|_| RegistryError::Factory("guild settings are unavailable".into()))?
        };
        let live = self.settings_persistence.latest(guild_id).await;
        let stored = match live.as_ref() {
            Some(view) => newest_durable(stored, view.durable.clone()),
            None => stored,
        };
        let stored_hrir = stored
            .default_hrir_preset_id
            .clone()
            .filter(|preset| self.catalog.get(preset).is_some())
            .unwrap_or(default_preset);
        let pending = live
            .and_then(|view| view.pending)
            .filter(|settings| self.catalog.get(&settings.hrir_preset).is_some());
        Ok(InitialSettings {
            volume: pending
                .as_ref()
                .map_or(stored.volume, |settings| settings.volume),
            idle_disconnect: stored.idle_disconnect,
            hrir_preset: pending
                .as_ref()
                .map_or(stored_hrir, |settings| settings.hrir_preset.clone()),
            spatial_audio_enabled: pending
                .as_ref()
                .map_or(stored.spatial_audio_enabled, |settings| {
                    settings.spatial_audio_enabled
                }),
            durable_seed: stored,
        })
    }
}

fn newest_durable(first: GuildSettings, second: GuildSettings) -> GuildSettings {
    if second.revision > first.revision {
        second
    } else {
        first
    }
}

#[async_trait]
impl PlayerFactory for ProductionPlayerFactory {
    async fn create(&self, guild_id: GuildId) -> Result<PlayerHandle, RegistryError> {
        let settings = self.settings(guild_id).await?;
        let durable_revision = self
            .valkey
            .get_snapshot_revision(guild_id)
            .await
            .map_err(|_| RegistryError::Factory("snapshot revision is unavailable".into()))?;
        let volatile_revision = self.snapshots.latest_revision(guild_id).await;
        let initial_revision = next_actor_revision(durable_revision, volatile_revision)
            .ok_or_else(|| RegistryError::Factory("snapshot revision is exhausted".into()))?;
        let initial_hrir = self
            .catalog
            .get(&settings.hrir_preset)
            .ok_or_else(|| RegistryError::Factory("configured HRIR is unavailable".into()))?;
        let dependencies = PipelineDependencies::new(
            Arc::clone(&self.resolver),
            self.decoder.clone(),
            Arc::clone(&self.hrirs),
            initial_hrir,
        );
        let playback = SongbirdPlayback::new(
            self.manager.clone(),
            guild_id,
            dependencies,
            PipelineConfig::default(),
        )
        .map_err(|_| RegistryError::Factory("audio pipeline initialization failed".into()))?;
        let events = playback.subscribe_events();
        let maximum_queue = usize::try_from(self.defaults.max_queue_items.get())
            .map_err(|_| RegistryError::Factory("queue limit is out of range".into()))?;
        let config = PlayerConfig::new(64, 128, maximum_queue, settings.idle_disconnect)
            .map_err(|error| RegistryError::Factory(error.to_string()))?;
        let snapshot_publisher = self
            .snapshots
            .publisher(guild_id)
            .await
            .map_err(|_| RegistryError::Factory("snapshot publisher is unavailable".into()))?;
        let settings_publisher = self
            .settings_persistence
            .publisher(
                guild_id,
                settings.durable_seed.clone(),
                settings.persistent(),
            )
            .await
            .map_err(|_| RegistryError::Factory("settings publisher is unavailable".into()))?;
        let publisher = PersistentSnapshotPublisher::new(snapshot_publisher, settings_publisher);
        let player =
            spawn_player_with_revision(guild_id, initial_revision, config, playback, publisher);
        let handle = player.handle();
        initialize(&handle, settings).await?;
        crate::production_event_bridge::spawn(guild_id, events, handle.clone());
        Ok(handle)
    }
}

fn next_actor_revision(
    durable: Option<StateRevision>,
    volatile: Option<StateRevision>,
) -> Option<StateRevision> {
    durable
        .into_iter()
        .chain(volatile)
        .max()
        .map_or(Some(StateRevision::INITIAL), StateRevision::checked_next)
}

#[async_trait]
impl pepeaudio_runtime::PlayerDirectory for PlayerRegistry {
    async fn player(&self, guild_id: GuildId) -> Result<Option<PlayerHandle>, WorkerPlayerError> {
        self.get_or_create(guild_id)
            .await
            .map(Some)
            .map_err(|_| WorkerPlayerError::Unavailable)
    }
}

struct InitialSettings {
    volume: pepeaudio_core::Volume,
    idle_disconnect: Duration,
    hrir_preset: pepeaudio_core::HrirPresetId,
    spatial_audio_enabled: bool,
    durable_seed: GuildSettings,
}

impl InitialSettings {
    fn persistent(&self) -> PersistentPlayerSettings {
        PersistentPlayerSettings {
            volume: self.volume,
            hrir_preset: self.hrir_preset.clone(),
            spatial_audio_enabled: self.spatial_audio_enabled,
        }
    }
}

async fn initialize(player: &PlayerHandle, settings: InitialSettings) -> Result<(), RegistryError> {
    let initial = player
        .snapshot()
        .await
        .map_err(|_| RegistryError::Factory("player initialization failed".into()))?;
    let guild_id = initial.guild_id;
    let mut revision = initial.revision;
    for command in [
        PlayerCommand::SetVolume {
            volume: settings.volume,
        },
        PlayerCommand::SetHrir {
            preset: settings.hrir_preset,
        },
        PlayerCommand::SetSpatialAudio {
            enabled: settings.spatial_audio_enabled,
        },
    ] {
        let envelope = CommandEnvelope::new(
            guild_id,
            None,
            revision,
            UnixTimeMillis::new(u64::MAX),
            command,
        );
        revision = player
            .apply(envelope)
            .await
            .map_err(|_| RegistryError::Factory("player initialization failed".into()))?
            .revision;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use pepeaudio_core::{GuildId, HrirPresetId, StateRevision, Volume};
    use pepeaudio_storage::{ControlPolicy, GuildSettings, SettingsRevision};
    use time::OffsetDateTime;

    use super::{newest_durable, next_actor_revision};

    #[test]
    fn recreated_actor_starts_after_durable_or_retrying_watermark() {
        assert_eq!(
            next_actor_revision(Some(StateRevision::new(5)), Some(StateRevision::new(9))),
            Some(StateRevision::new(10))
        );
        assert_eq!(
            next_actor_revision(Some(StateRevision::new(9)), Some(StateRevision::new(5))),
            Some(StateRevision::new(10))
        );
        assert_eq!(
            next_actor_revision(None, None),
            Some(StateRevision::INITIAL)
        );
        assert_eq!(
            next_actor_revision(Some(StateRevision::new(u64::MAX)), None),
            None
        );
    }

    #[test]
    fn actor_recreation_uses_the_newest_confirmed_settings_row() {
        let older = settings_row(4, 25);
        let newer = settings_row(5, 80);

        assert_eq!(newest_durable(older, newer).volume.percent(), 80);
    }

    fn settings_row(revision: u64, volume: u8) -> GuildSettings {
        GuildSettings {
            guild_id: GuildId::new(1).expect("guild"),
            volume: Volume::new(volume).expect("volume"),
            idle_disconnect: Duration::from_mins(5),
            control_policy: ControlPolicy::SameVoiceChannel,
            dj_role_id: None,
            default_hrir_preset_id: Some(HrirPresetId::new("default").expect("HRIR")),
            spatial_audio_enabled: false,
            revision: SettingsRevision::new(revision),
            created_at: OffsetDateTime::UNIX_EPOCH,
            updated_at: OffsetDateTime::UNIX_EPOCH,
        }
    }
}
