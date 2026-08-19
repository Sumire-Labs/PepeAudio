use async_trait::async_trait;
use pepeaudio_core::{GuildId, UserId};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use super::{
    PostgresStorage,
    rows::{PlaylistRow, PlaylistTrackRow, checked_i64},
};
use crate::{
    Playlist, PlaylistRepository, PlaylistTrack, SettingsRevision, StorageError, StorageResult,
};

#[async_trait]
impl PlaylistRepository for PostgresStorage {
    async fn create_playlist(
        &self,
        playlist: &Playlist,
        tracks: &[PlaylistTrack],
    ) -> StorageResult<Playlist> {
        let mut transaction = self.pool.begin().await?;
        let revision = checked_i64(playlist.revision.get(), "playlist", "revision")?;
        let row = sqlx::query_as::<_, PlaylistRow>(
            r"
            INSERT INTO playlists (
                playlist_id, guild_id, owner_user_id, name, description,
                visibility, revision
            ) VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING playlist_id, guild_id, owner_user_id, name, description,
                      visibility, revision, created_at, updated_at
            ",
        )
        .bind(playlist.playlist_id)
        .bind(playlist.guild_id.to_string())
        .bind(playlist.owner_user_id.to_string())
        .bind(&playlist.name)
        .bind(&playlist.description)
        .bind(playlist.visibility.as_db())
        .bind(revision)
        .fetch_one(&mut *transaction)
        .await?;
        insert_tracks(&mut transaction, playlist.playlist_id, tracks).await?;
        transaction.commit().await?;
        Playlist::try_from(row)
    }

    async fn get_playlist(
        &self,
        playlist_id: Uuid,
    ) -> StorageResult<Option<(Playlist, Vec<PlaylistTrack>)>> {
        let Some(row) = sqlx::query_as::<_, PlaylistRow>(
            r"
            SELECT playlist_id, guild_id, owner_user_id, name, description,
                   visibility, revision, created_at, updated_at
            FROM playlists WHERE playlist_id = $1
            ",
        )
        .bind(playlist_id)
        .fetch_optional(&self.pool)
        .await?
        else {
            return Ok(None);
        };
        let tracks = sqlx::query_as::<_, PlaylistTrackRow>(
            r"
            SELECT track_id, position, source_kind, source_reference, title,
                   duration_ms, added_by_user_id, created_at
            FROM playlist_tracks
            WHERE playlist_id = $1 ORDER BY position
            ",
        )
        .bind(playlist_id)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(PlaylistTrack::try_from)
        .collect::<StorageResult<Vec<_>>>()?;
        Ok(Some((Playlist::try_from(row)?, tracks)))
    }

    async fn list_playlists(
        &self,
        guild_id: GuildId,
        viewer_user_id: UserId,
    ) -> StorageResult<Vec<Playlist>> {
        let rows = sqlx::query_as::<_, PlaylistRow>(
            r"
            SELECT playlist_id, guild_id, owner_user_id, name, description,
                   visibility, revision, created_at, updated_at
            FROM playlists
            WHERE guild_id = $1 AND (visibility = 'guild' OR owner_user_id = $2)
            ORDER BY updated_at DESC, playlist_id
            ",
        )
        .bind(guild_id.to_string())
        .bind(viewer_user_id.to_string())
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(Playlist::try_from).collect()
    }

    async fn replace_playlist_tracks(
        &self,
        playlist_id: Uuid,
        expected_revision: SettingsRevision,
        tracks: &[PlaylistTrack],
    ) -> StorageResult<Option<Playlist>> {
        let mut transaction = self.pool.begin().await?;
        let expected = checked_i64(expected_revision.get(), "playlist", "revision")?;
        let updated = sqlx::query_as::<_, PlaylistRow>(
            r"
            UPDATE playlists SET revision = revision + 1, updated_at = now()
            WHERE playlist_id = $1 AND revision = $2
            RETURNING playlist_id, guild_id, owner_user_id, name, description,
                      visibility, revision, created_at, updated_at
            ",
        )
        .bind(playlist_id)
        .bind(expected)
        .fetch_optional(&mut *transaction)
        .await?;
        let Some(updated) = updated else {
            transaction.rollback().await?;
            return Ok(None);
        };

        sqlx::query("DELETE FROM playlist_tracks WHERE playlist_id = $1")
            .bind(playlist_id)
            .execute(&mut *transaction)
            .await?;
        insert_tracks(&mut transaction, playlist_id, tracks).await?;
        transaction.commit().await?;
        Ok(Some(Playlist::try_from(updated)?))
    }
}

async fn insert_tracks(
    transaction: &mut Transaction<'_, Postgres>,
    playlist_id: Uuid,
    tracks: &[PlaylistTrack],
) -> StorageResult<()> {
    for (expected_position, track) in tracks.iter().enumerate() {
        if usize::try_from(track.position).ok() != Some(expected_position) {
            return Err(StorageError::CorruptData {
                entity: "playlist_track",
                field: "position",
            });
        }
        let position = i32::try_from(track.position).map_err(|_| StorageError::CorruptData {
            entity: "playlist_track",
            field: "position",
        })?;
        let duration_ms = track
            .duration_ms
            .map(|value| checked_i64(value, "playlist_track", "duration_ms"))
            .transpose()?;
        sqlx::query(
            r"
            INSERT INTO playlist_tracks (
                track_id, playlist_id, position, source_kind, source_reference,
                title, duration_ms, added_by_user_id
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ",
        )
        .bind(track.track_id)
        .bind(playlist_id)
        .bind(position)
        .bind(track.source_kind.as_db())
        .bind(&track.source_reference)
        .bind(&track.title)
        .bind(duration_ms)
        .bind(track.added_by_user_id.to_string())
        .execute(&mut **transaction)
        .await?;
    }
    Ok(())
}
