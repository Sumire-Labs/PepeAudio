use pepeaudio_core::{HrirPresetId, PlayerSnapshot, StateRevision, Volume};
use pepeaudio_storage::GuildSettings;

/// The subset of guild defaults updated by live player controls.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistentPlayerSettings {
    pub volume: Volume,
    pub hrir_preset: HrirPresetId,
    pub spatial_audio_enabled: bool,
}

impl PersistentPlayerSettings {
    #[must_use]
    pub fn from_snapshot(snapshot: &PlayerSnapshot) -> Option<Self> {
        Some(Self {
            volume: snapshot.volume,
            hrir_preset: snapshot.hrir_preset.clone()?,
            spatial_audio_enabled: snapshot.spatial_audio_enabled,
        })
    }

    #[must_use]
    pub fn from_guild_settings(settings: &GuildSettings) -> Option<Self> {
        Some(Self {
            volume: settings.volume,
            hrir_preset: settings.default_hrir_preset_id.clone()?,
            spatial_audio_enabled: settings.spatial_audio_enabled,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SettingsUpdate {
    pub(crate) actor_revision: StateRevision,
    pub(crate) settings: PersistentPlayerSettings,
}

/// Coherent durable and pending values observed by actor recreation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SettingsPersistenceView {
    pub durable: GuildSettings,
    /// Newer actor value which is still being persisted, if any.
    pub pending: Option<PersistentPlayerSettings>,
}

#[derive(Clone, Debug)]
pub(crate) struct SettingsWorkerState {
    pub(crate) durable: GuildSettings,
    pub(crate) pending: Option<SettingsUpdate>,
}

impl SettingsWorkerState {
    pub(crate) fn view(&self) -> SettingsPersistenceView {
        SettingsPersistenceView {
            durable: self.durable.clone(),
            pending: self.pending.as_ref().map(|update| update.settings.clone()),
        }
    }
}
