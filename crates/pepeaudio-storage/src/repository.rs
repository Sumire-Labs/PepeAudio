use async_trait::async_trait;
use pepeaudio_core::{GuildId, HrirPresetId, UserId};
use uuid::Uuid;

use crate::{
    GuildSettings, HrirPresetMetadata, Playlist, PlaylistTrack, SettingsRevision, StorageResult,
};

#[async_trait]
pub trait GuildSettingsRepository: Send + Sync {
    /// Gets settings, or returns `None` before the guild has been initialized.
    async fn get_guild_settings(&self, guild_id: GuildId) -> StorageResult<Option<GuildSettings>>;

    /// Inserts defaults only when absent, then returns the authoritative row.
    async fn create_guild_settings(&self, defaults: &GuildSettings)
    -> StorageResult<GuildSettings>;

    /// Replaces settings only at `expected_revision`, incrementing it atomically.
    async fn update_guild_settings(
        &self,
        settings: &GuildSettings,
        expected_revision: SettingsRevision,
    ) -> StorageResult<Option<GuildSettings>>;
}

/// Binary impulse data is never accepted through this repository.
#[async_trait]
pub trait HrirPresetRepository: Send + Sync {
    async fn get_hrir_preset(
        &self,
        preset_id: &HrirPresetId,
    ) -> StorageResult<Option<HrirPresetMetadata>>;

    /// Lists global presets plus presets owned by the supplied guild.
    async fn list_hrir_presets(&self, guild_id: GuildId) -> StorageResult<Vec<HrirPresetMetadata>>;

    async fn insert_hrir_preset(
        &self,
        preset: &HrirPresetMetadata,
    ) -> StorageResult<HrirPresetMetadata>;

    /// Deletes a preset only if it is owned by `guild_id`.
    async fn delete_hrir_preset(
        &self,
        preset_id: &HrirPresetId,
        guild_id: GuildId,
    ) -> StorageResult<bool>;
}

#[async_trait]
pub trait PlaylistRepository: Send + Sync {
    /// Creates a playlist and its ordered items in one transaction.
    async fn create_playlist(
        &self,
        playlist: &Playlist,
        tracks: &[PlaylistTrack],
    ) -> StorageResult<Playlist>;

    async fn get_playlist(
        &self,
        playlist_id: Uuid,
    ) -> StorageResult<Option<(Playlist, Vec<PlaylistTrack>)>>;

    /// Lists guild-visible playlists plus private playlists owned by the viewer.
    async fn list_playlists(
        &self,
        guild_id: GuildId,
        viewer_user_id: UserId,
    ) -> StorageResult<Vec<Playlist>>;

    /// Atomically replaces ordered items when the playlist revision matches.
    async fn replace_playlist_tracks(
        &self,
        playlist_id: Uuid,
        expected_revision: SettingsRevision,
        tracks: &[PlaylistTrack],
    ) -> StorageResult<Option<Playlist>>;
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use async_trait::async_trait;
    use pepeaudio_core::GuildId;

    use super::GuildSettingsRepository;
    use crate::{GuildSettings, SettingsRevision, StorageResult};

    #[derive(Default)]
    struct FakeSettingsRepository(Mutex<Option<GuildSettings>>);

    #[async_trait]
    impl GuildSettingsRepository for FakeSettingsRepository {
        async fn get_guild_settings(
            &self,
            _guild_id: GuildId,
        ) -> StorageResult<Option<GuildSettings>> {
            Ok(self.0.lock().expect("fake lock").clone())
        }

        async fn create_guild_settings(
            &self,
            defaults: &GuildSettings,
        ) -> StorageResult<GuildSettings> {
            let mut current = self.0.lock().expect("fake lock");
            Ok(current.get_or_insert_with(|| defaults.clone()).clone())
        }

        async fn update_guild_settings(
            &self,
            settings: &GuildSettings,
            expected_revision: SettingsRevision,
        ) -> StorageResult<Option<GuildSettings>> {
            let mut current = self.0.lock().expect("fake lock");
            if current.as_ref().map(|item| item.revision) != Some(expected_revision) {
                return Ok(None);
            }
            *current = Some(settings.clone());
            Ok(current.clone())
        }
    }

    #[test]
    fn repository_interface_can_be_replaced_by_a_memory_fake() {
        fn accepts_repository(_: &impl GuildSettingsRepository) {}

        accepts_repository(&FakeSettingsRepository::default());
    }
}
