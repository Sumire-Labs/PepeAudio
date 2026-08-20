use async_trait::async_trait;
use pepeaudio_core::{GuildId, HrirPresetId};

use super::{
    PostgresStorage,
    rows::{HrirPresetRow, checked_i64},
};
use crate::{HrirPresetMetadata, HrirPresetRepository, StorageResult};

impl PostgresStorage {
    /// Upserts the complete operator-installed global catalog and removes
    /// global rows whose files are no longer installed. License and
    /// attribution fields already curated in `PostgreSQL` are preserved.
    ///
    /// # Errors
    ///
    /// Returns an error for a guild-owned input, invalid numeric bound, or
    /// failed atomic transaction.
    pub async fn synchronize_global_hrir_presets(
        &self,
        presets: &[HrirPresetMetadata],
    ) -> StorageResult<()> {
        let mut transaction = self.pool.begin().await?;
        let mut installed_ids = Vec::with_capacity(presets.len());
        for preset in presets {
            if preset.owner_guild_id.is_some() {
                return Err(crate::StorageError::CorruptData {
                    entity: "hrir_preset",
                    field: "owner_guild_id",
                });
            }
            let file_size = checked_i64(preset.file_size_bytes, "hrir_preset", "file_size_bytes")?;
            let result = sqlx::query(
                r"
                INSERT INTO hrir_presets (
                    preset_id, owner_guild_id, display_name, description, storage_key, sha256_hex,
                    sample_rate, channel_layout, file_size_bytes, license_name,
                    license_url, attribution
                ) VALUES ($1, NULL, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
                ON CONFLICT (preset_id) DO UPDATE SET
                    display_name = EXCLUDED.display_name,
                    description = EXCLUDED.description,
                    storage_key = EXCLUDED.storage_key,
                    sha256_hex = EXCLUDED.sha256_hex,
                    sample_rate = EXCLUDED.sample_rate,
                    channel_layout = EXCLUDED.channel_layout,
                    file_size_bytes = EXCLUDED.file_size_bytes
                WHERE hrir_presets.owner_guild_id IS NULL
                ",
            )
            .bind(preset.preset_id.as_str())
            .bind(&preset.display_name)
            .bind(&preset.description)
            .bind(&preset.storage_key)
            .bind(&preset.sha256_hex)
            .bind(i32::try_from(preset.sample_rate).map_err(|_| {
                crate::StorageError::CorruptData {
                    entity: "hrir_preset",
                    field: "sample_rate",
                }
            })?)
            .bind(preset.channel_layout.as_db())
            .bind(file_size)
            .bind(&preset.license_name)
            .bind(&preset.license_url)
            .bind(&preset.attribution)
            .execute(&mut *transaction)
            .await?;
            if result.rows_affected() != 1 {
                return Err(crate::StorageError::CorruptData {
                    entity: "hrir_preset",
                    field: "preset_id",
                });
            }
            installed_ids.push(preset.preset_id.to_string());
        }
        sqlx::query(
            "DELETE FROM hrir_presets
             WHERE owner_guild_id IS NULL AND NOT (preset_id = ANY($1))",
        )
        .bind(&installed_ids)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(())
    }
}

#[async_trait]
impl HrirPresetRepository for PostgresStorage {
    async fn get_hrir_preset(
        &self,
        preset_id: &HrirPresetId,
    ) -> StorageResult<Option<HrirPresetMetadata>> {
        let row = sqlx::query_as::<_, HrirPresetRow>(
            r"
            SELECT preset_id, owner_guild_id, display_name, description, storage_key, sha256_hex,
                   sample_rate, channel_layout, file_size_bytes, license_name,
                   license_url, attribution, created_at
            FROM hrir_presets WHERE preset_id = $1
            ",
        )
        .bind(preset_id.as_str())
        .fetch_optional(&self.pool)
        .await?;
        row.map(HrirPresetMetadata::try_from).transpose()
    }

    async fn list_hrir_presets(&self, guild_id: GuildId) -> StorageResult<Vec<HrirPresetMetadata>> {
        let rows = sqlx::query_as::<_, HrirPresetRow>(
            r#"
            SELECT preset_id, owner_guild_id, display_name, description, storage_key, sha256_hex,
                   sample_rate, channel_layout, file_size_bytes, license_name,
                   license_url, attribution, created_at
            FROM hrir_presets
            WHERE owner_guild_id IS NULL OR owner_guild_id = $1
            ORDER BY display_name COLLATE "C", preset_id COLLATE "C"
            "#,
        )
        .bind(guild_id.to_string())
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(HrirPresetMetadata::try_from).collect()
    }

    async fn insert_hrir_preset(
        &self,
        preset: &HrirPresetMetadata,
    ) -> StorageResult<HrirPresetMetadata> {
        let file_size = checked_i64(preset.file_size_bytes, "hrir_preset", "file_size_bytes")?;
        let row = sqlx::query_as::<_, HrirPresetRow>(
            r"
            INSERT INTO hrir_presets (
                preset_id, owner_guild_id, display_name, description, storage_key, sha256_hex,
                sample_rate, channel_layout, file_size_bytes, license_name,
                license_url, attribution
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            RETURNING preset_id, owner_guild_id, display_name, description, storage_key, sha256_hex,
                      sample_rate, channel_layout, file_size_bytes, license_name,
                      license_url, attribution, created_at
            ",
        )
        .bind(preset.preset_id.as_str())
        .bind(preset.owner_guild_id.map(|id| id.to_string()))
        .bind(&preset.display_name)
        .bind(&preset.description)
        .bind(&preset.storage_key)
        .bind(&preset.sha256_hex)
        .bind(
            i32::try_from(preset.sample_rate).map_err(|_| crate::StorageError::CorruptData {
                entity: "hrir_preset",
                field: "sample_rate",
            })?,
        )
        .bind(preset.channel_layout.as_db())
        .bind(file_size)
        .bind(&preset.license_name)
        .bind(&preset.license_url)
        .bind(&preset.attribution)
        .fetch_one(&self.pool)
        .await?;
        HrirPresetMetadata::try_from(row)
    }

    async fn delete_hrir_preset(
        &self,
        preset_id: &HrirPresetId,
        guild_id: GuildId,
    ) -> StorageResult<bool> {
        let result =
            sqlx::query("DELETE FROM hrir_presets WHERE preset_id = $1 AND owner_guild_id = $2")
                .bind(preset_id.as_str())
                .bind(guild_id.to_string())
                .execute(&self.pool)
                .await?;
        Ok(result.rows_affected() == 1)
    }
}
