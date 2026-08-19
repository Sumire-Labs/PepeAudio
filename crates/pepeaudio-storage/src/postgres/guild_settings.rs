use std::time::Duration;

use async_trait::async_trait;
use pepeaudio_core::GuildId;

use super::{
    PostgresStorage,
    rows::{GuildSettingsRow, checked_i32, checked_i64},
};
use crate::{
    GuildSettings, GuildSettingsRepository, SettingsRevision, StorageError, StorageResult,
};

#[async_trait]
impl GuildSettingsRepository for PostgresStorage {
    async fn get_guild_settings(&self, guild_id: GuildId) -> StorageResult<Option<GuildSettings>> {
        let row = sqlx::query_as::<_, GuildSettingsRow>(
            r"
            SELECT guild_id, volume_percent, idle_disconnect_seconds, control_policy,
                   dj_role_id, default_hrir_preset_id, spatial_audio_enabled,
                   revision, created_at, updated_at
            FROM guild_settings WHERE guild_id = $1
            ",
        )
        .bind(guild_id.to_string())
        .fetch_optional(&self.pool)
        .await?;
        row.map(GuildSettings::try_from).transpose()
    }

    async fn create_guild_settings(
        &self,
        defaults: &GuildSettings,
    ) -> StorageResult<GuildSettings> {
        let idle = duration_seconds(defaults.idle_disconnect)?;
        let revision = checked_i64(defaults.revision.get(), "guild_settings", "revision")?;
        let row = sqlx::query_as::<_, GuildSettingsRow>(
            r"
            INSERT INTO guild_settings (
                guild_id, volume_percent, idle_disconnect_seconds, control_policy,
                dj_role_id, default_hrir_preset_id, spatial_audio_enabled, revision
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (guild_id) DO NOTHING
            RETURNING guild_id, volume_percent, idle_disconnect_seconds, control_policy,
                      dj_role_id, default_hrir_preset_id, spatial_audio_enabled,
                      revision, created_at, updated_at
            ",
        )
        .bind(defaults.guild_id.to_string())
        .bind(i16::from(defaults.volume.percent()))
        .bind(idle)
        .bind(defaults.control_policy.as_db())
        .bind(defaults.dj_role_id.map(|id| id.to_string()))
        .bind(
            defaults
                .default_hrir_preset_id
                .as_ref()
                .map(pepeaudio_core::HrirPresetId::as_str),
        )
        .bind(defaults.spatial_audio_enabled)
        .bind(revision)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            GuildSettings::try_from(row)
        } else {
            self.get_guild_settings(defaults.guild_id)
                .await?
                .ok_or(StorageError::CorruptData {
                    entity: "guild_settings",
                    field: "guild_id",
                })
        }
    }

    async fn update_guild_settings(
        &self,
        settings: &GuildSettings,
        expected_revision: SettingsRevision,
    ) -> StorageResult<Option<GuildSettings>> {
        let idle = duration_seconds(settings.idle_disconnect)?;
        let expected = checked_i64(expected_revision.get(), "guild_settings", "revision")?;
        let row = sqlx::query_as::<_, GuildSettingsRow>(
            r"
            UPDATE guild_settings SET
                volume_percent = $2,
                idle_disconnect_seconds = $3,
                control_policy = $4,
                dj_role_id = $5,
                default_hrir_preset_id = $6,
                spatial_audio_enabled = $7,
                revision = revision + 1,
                updated_at = now()
            WHERE guild_id = $1 AND revision = $8
            RETURNING guild_id, volume_percent, idle_disconnect_seconds, control_policy,
                      dj_role_id, default_hrir_preset_id, spatial_audio_enabled,
                      revision, created_at, updated_at
            ",
        )
        .bind(settings.guild_id.to_string())
        .bind(i16::from(settings.volume.percent()))
        .bind(idle)
        .bind(settings.control_policy.as_db())
        .bind(settings.dj_role_id.map(|id| id.to_string()))
        .bind(
            settings
                .default_hrir_preset_id
                .as_ref()
                .map(pepeaudio_core::HrirPresetId::as_str),
        )
        .bind(settings.spatial_audio_enabled)
        .bind(expected)
        .fetch_optional(&self.pool)
        .await?;
        row.map(GuildSettings::try_from).transpose()
    }
}

fn duration_seconds(duration: Duration) -> StorageResult<i32> {
    checked_i32(
        duration.as_secs(),
        "guild_settings",
        "idle_disconnect_seconds",
    )
}
