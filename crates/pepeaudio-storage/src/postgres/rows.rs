use std::time::Duration;

use pepeaudio_core::{HrirPresetId, Volume};
use sqlx::FromRow;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    ControlPolicy, GuildSettings, HrirChannelLayout, HrirPresetMetadata, Playlist, PlaylistTrack,
    PlaylistVisibility, SettingsRevision, StorageError, StorageResult, TrackSourceKind,
};

#[derive(FromRow)]
pub(super) struct GuildSettingsRow {
    guild_id: String,
    volume_percent: i16,
    idle_disconnect_seconds: i32,
    control_policy: String,
    dj_role_id: Option<String>,
    default_hrir_preset_id: Option<String>,
    spatial_audio_enabled: bool,
    revision: i64,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

impl TryFrom<GuildSettingsRow> for GuildSettings {
    type Error = StorageError;

    fn try_from(row: GuildSettingsRow) -> Result<Self, Self::Error> {
        Ok(Self {
            guild_id: parse_id(&row.guild_id, "guild_settings", "guild_id")?,
            volume: Volume::new(
                u8::try_from(row.volume_percent)
                    .map_err(|_| corrupt("guild_settings", "volume_percent"))?,
            )
            .map_err(|_| corrupt("guild_settings", "volume_percent"))?,
            idle_disconnect: Duration::from_secs(
                u64::try_from(row.idle_disconnect_seconds)
                    .map_err(|_| corrupt("guild_settings", "idle_disconnect_seconds"))?,
            ),
            control_policy: ControlPolicy::from_db(&row.control_policy)
                .ok_or_else(|| corrupt("guild_settings", "control_policy"))?,
            dj_role_id: row
                .dj_role_id
                .map(|value| parse_nonzero_u64(&value, "guild_settings", "dj_role_id"))
                .transpose()?,
            default_hrir_preset_id: row
                .default_hrir_preset_id
                .map(HrirPresetId::new)
                .transpose()
                .map_err(|_| corrupt("guild_settings", "default_hrir_preset_id"))?,
            spatial_audio_enabled: row.spatial_audio_enabled,
            revision: revision(row.revision, "guild_settings")?,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
}

#[derive(FromRow)]
pub(super) struct HrirPresetRow {
    preset_id: String,
    owner_guild_id: Option<String>,
    display_name: String,
    description: Option<String>,
    storage_key: String,
    sha256_hex: String,
    sample_rate: i32,
    channel_layout: String,
    file_size_bytes: i64,
    license_name: Option<String>,
    license_url: Option<String>,
    attribution: Option<String>,
    created_at: OffsetDateTime,
}

impl TryFrom<HrirPresetRow> for HrirPresetMetadata {
    type Error = StorageError;

    fn try_from(row: HrirPresetRow) -> Result<Self, Self::Error> {
        Ok(Self {
            preset_id: HrirPresetId::new(row.preset_id)
                .map_err(|_| corrupt("hrir_preset", "preset_id"))?,
            owner_guild_id: row
                .owner_guild_id
                .map(|value| parse_id(&value, "hrir_preset", "owner_guild_id"))
                .transpose()?,
            display_name: row.display_name,
            description: row.description,
            storage_key: row.storage_key,
            sha256_hex: row.sha256_hex,
            sample_rate: u32::try_from(row.sample_rate)
                .map_err(|_| corrupt("hrir_preset", "sample_rate"))?,
            channel_layout: HrirChannelLayout::from_db(&row.channel_layout)
                .ok_or_else(|| corrupt("hrir_preset", "channel_layout"))?,
            file_size_bytes: u64::try_from(row.file_size_bytes)
                .map_err(|_| corrupt("hrir_preset", "file_size_bytes"))?,
            license_name: row.license_name,
            license_url: row.license_url,
            attribution: row.attribution,
            created_at: row.created_at,
        })
    }
}

#[derive(FromRow)]
pub(super) struct PlaylistRow {
    playlist_id: Uuid,
    guild_id: String,
    owner_user_id: String,
    name: String,
    description: Option<String>,
    visibility: String,
    revision: i64,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

impl TryFrom<PlaylistRow> for Playlist {
    type Error = StorageError;

    fn try_from(row: PlaylistRow) -> Result<Self, Self::Error> {
        Ok(Self {
            playlist_id: row.playlist_id,
            guild_id: parse_id(&row.guild_id, "playlist", "guild_id")?,
            owner_user_id: parse_id(&row.owner_user_id, "playlist", "owner_user_id")?,
            name: row.name,
            description: row.description,
            visibility: PlaylistVisibility::from_db(&row.visibility)
                .ok_or_else(|| corrupt("playlist", "visibility"))?,
            revision: revision(row.revision, "playlist")?,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
}

#[derive(FromRow)]
pub(super) struct PlaylistTrackRow {
    track_id: Uuid,
    position: i32,
    source_kind: String,
    source_reference: String,
    title: String,
    duration_ms: Option<i64>,
    added_by_user_id: String,
    created_at: OffsetDateTime,
}

impl TryFrom<PlaylistTrackRow> for PlaylistTrack {
    type Error = StorageError;

    fn try_from(row: PlaylistTrackRow) -> Result<Self, Self::Error> {
        Ok(Self {
            track_id: row.track_id,
            position: u32::try_from(row.position)
                .map_err(|_| corrupt("playlist_track", "position"))?,
            source_kind: TrackSourceKind::from_db(&row.source_kind)
                .ok_or_else(|| corrupt("playlist_track", "source_kind"))?,
            source_reference: row.source_reference,
            title: row.title,
            duration_ms: row
                .duration_ms
                .map(|value| {
                    u64::try_from(value).map_err(|_| corrupt("playlist_track", "duration_ms"))
                })
                .transpose()?,
            added_by_user_id: parse_id(
                &row.added_by_user_id,
                "playlist_track",
                "added_by_user_id",
            )?,
            created_at: row.created_at,
        })
    }
}

pub(super) fn checked_i64(
    value: u64,
    entity: &'static str,
    field: &'static str,
) -> StorageResult<i64> {
    i64::try_from(value).map_err(|_| corrupt(entity, field))
}

pub(super) fn checked_i32(
    value: u64,
    entity: &'static str,
    field: &'static str,
) -> StorageResult<i32> {
    i32::try_from(value).map_err(|_| corrupt(entity, field))
}

fn revision(value: i64, entity: &'static str) -> StorageResult<SettingsRevision> {
    Ok(SettingsRevision::new(
        u64::try_from(value).map_err(|_| corrupt(entity, "revision"))?,
    ))
}

fn parse_id<T>(value: &str, entity: &'static str, field: &'static str) -> StorageResult<T>
where
    T: std::str::FromStr,
{
    value.parse().map_err(|_| corrupt(entity, field))
}

fn parse_nonzero_u64(value: &str, entity: &'static str, field: &'static str) -> StorageResult<u64> {
    let parsed = value.parse::<u64>().map_err(|_| corrupt(entity, field))?;
    if parsed == 0 {
        Err(corrupt(entity, field))
    } else {
        Ok(parsed)
    }
}

const fn corrupt(entity: &'static str, field: &'static str) -> StorageError {
    StorageError::CorruptData { entity, field }
}
