use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    ChannelId, GuildId, HrirPresetId, StateRevision, TrackProvenance, UnixTimeMillis, UserId,
    Volume,
};

/// Maximum UTF-8 size of a track title exposed through public snapshots.
pub const MAX_TRACK_TITLE_BYTES: usize = 512;
/// Maximum UTF-8 size of an optional display artist.
pub const MAX_TRACK_ARTIST_BYTES: usize = 256;
/// Maximum UTF-8 size of an optional album or release name.
pub const MAX_TRACK_ALBUM_BYTES: usize = 512;
/// Maximum number of upcoming entries carried by one authoritative snapshot.
pub const MAX_SNAPSHOT_QUEUE_ITEMS: usize = 100;
/// Hard transport and cache bound for one serialized player snapshot.
pub const MAX_PLAYER_SNAPSHOT_JSON_BYTES: usize = 1_048_576;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlayerState {
    Disconnected,
    IdleConnected,
    Loading,
    Playing,
    Paused,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RepeatMode {
    #[default]
    Off,
    Track,
    Queue,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TrackSnapshot {
    pub track_id: Uuid,
    pub title: String,
    /// Optional display artist. Missing in snapshots created before metadata
    /// enrichment and for direct media without trustworthy attribution.
    #[serde(default)]
    pub artist: Option<String>,
    /// Optional release or album display name.
    #[serde(default)]
    pub album: Option<String>,
    /// Stable public source pages only. Playback locators are never included.
    #[serde(default)]
    pub provenance: Option<TrackProvenance>,
    /// User who requested the track, or `None` for a system insertion.
    pub requester_user_id: Option<UserId>,
    /// Known duration in milliseconds; absent for unknown or live media.
    pub duration_ms: Option<u64>,
    /// Last authoritative playback position in milliseconds.
    pub position_ms: u64,
    pub seekable: bool,
}

/// Authoritative player state shared with Discord and web front ends.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PlayerSnapshot {
    pub guild_id: GuildId,
    pub voice_channel_id: Option<ChannelId>,
    /// Optimistic-concurrency revision.
    pub revision: StateRevision,
    pub state: PlayerState,
    pub current_track: Option<TrackSnapshot>,
    /// Number of upcoming queue entries, excluding the current track.
    pub queued_tracks: u32,
    /// Upcoming tracks in their authoritative playback order. Media locators
    /// are intentionally excluded from this public view.
    #[serde(default)]
    pub upcoming_tracks: Vec<TrackSnapshot>,
    pub has_previous_track: bool,
    pub volume: Volume,
    pub repeat_mode: RepeatMode,
    pub shuffle_enabled: bool,
    pub hrir_preset: Option<HrirPresetId>,
    pub spatial_audio_enabled: bool,
    pub observed_at: UnixTimeMillis,
}

impl PlayerSnapshot {
    /// Verifies the bounded public shape accepted by cache and SSE adapters.
    ///
    /// # Errors
    ///
    /// Returns a field-specific error for inconsistent queue counts, duplicate
    /// identities, or unbounded provider-controlled display text.
    pub fn validate_public_shape(&self) -> Result<(), PlayerSnapshotValidationError> {
        if self.upcoming_tracks.len() > MAX_SNAPSHOT_QUEUE_ITEMS {
            return Err(PlayerSnapshotValidationError::QueueTooLarge);
        }
        if usize::try_from(self.queued_tracks).ok() != Some(self.upcoming_tracks.len()) {
            return Err(PlayerSnapshotValidationError::QueueCountMismatch);
        }

        let mut identities = HashSet::with_capacity(self.upcoming_tracks.len().saturating_add(1));
        for track in self.current_track.iter().chain(self.upcoming_tracks.iter()) {
            validate_track(track)?;
            if !identities.insert(track.track_id) {
                return Err(PlayerSnapshotValidationError::DuplicateTrackId);
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum PlayerSnapshotValidationError {
    #[error("player snapshot queue exceeds its public limit")]
    QueueTooLarge,
    #[error("player snapshot queue count is inconsistent")]
    QueueCountMismatch,
    #[error("player snapshot contains a duplicate track identity")]
    DuplicateTrackId,
    #[error("player snapshot track title is invalid")]
    InvalidTitle,
    #[error("player snapshot track artist is invalid")]
    InvalidArtist,
    #[error("player snapshot track album is invalid")]
    InvalidAlbum,
    #[error("player snapshot track position exceeds its duration")]
    InvalidPosition,
}

impl PlayerSnapshotValidationError {
    #[must_use]
    pub const fn field(self) -> &'static str {
        match self {
            Self::QueueTooLarge | Self::QueueCountMismatch => "upcoming_tracks",
            Self::DuplicateTrackId => "track_id",
            Self::InvalidTitle => "title",
            Self::InvalidArtist => "artist",
            Self::InvalidAlbum => "album",
            Self::InvalidPosition => "position_ms",
        }
    }
}

fn validate_track(track: &TrackSnapshot) -> Result<(), PlayerSnapshotValidationError> {
    validate_text(&track.title, MAX_TRACK_TITLE_BYTES)
        .then_some(())
        .ok_or(PlayerSnapshotValidationError::InvalidTitle)?;
    if track
        .artist
        .as_deref()
        .is_some_and(|value| !validate_text(value, MAX_TRACK_ARTIST_BYTES))
    {
        return Err(PlayerSnapshotValidationError::InvalidArtist);
    }
    if track
        .album
        .as_deref()
        .is_some_and(|value| !validate_text(value, MAX_TRACK_ALBUM_BYTES))
    {
        return Err(PlayerSnapshotValidationError::InvalidAlbum);
    }
    if track
        .duration_ms
        .is_some_and(|duration| track.position_ms > duration)
    {
        return Err(PlayerSnapshotValidationError::InvalidPosition);
    }
    Ok(())
}

fn validate_text(value: &str, maximum_bytes: usize) -> bool {
    !value.trim().is_empty() && value.len() <= maximum_bytes && !value.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_PLAYER_SNAPSHOT_JSON_BYTES, MAX_SNAPSHOT_QUEUE_ITEMS, MAX_TRACK_ALBUM_BYTES,
        MAX_TRACK_ARTIST_BYTES, MAX_TRACK_TITLE_BYTES, PlayerSnapshot,
        PlayerSnapshotValidationError, PlayerState, RepeatMode, TrackSnapshot,
    };
    use crate::{
        GuildId, MediaProvider, PublicMediaPage, StateRevision, TrackProvenance, UnixTimeMillis,
        Volume,
    };
    use uuid::Uuid;

    #[test]
    fn old_track_snapshot_without_display_metadata_still_deserializes() {
        let snapshot: TrackSnapshot = serde_json::from_value(serde_json::json!({
            "track_id": "00000000-0000-0000-0000-000000000001",
            "title": "Legacy track",
            "requester_user_id": null,
            "duration_ms": 120_000,
            "position_ms": 1_000,
            "seekable": true
        }))
        .expect("legacy snapshot");

        assert_eq!(snapshot.title, "Legacy track");
        assert_eq!(snapshot.artist, None);
        assert_eq!(snapshot.album, None);
        assert_eq!(snapshot.provenance, None);
    }

    #[test]
    fn maximum_public_queue_remains_below_the_transport_limit() {
        let mut snapshot = snapshot();
        let provenance = maximum_public_provenance();
        snapshot.upcoming_tracks = (0..MAX_SNAPSHOT_QUEUE_ITEMS)
            .map(|index| TrackSnapshot {
                track_id: Uuid::from_u128(index as u128 + 1),
                title: "\\".repeat(MAX_TRACK_TITLE_BYTES),
                artist: Some("\\".repeat(MAX_TRACK_ARTIST_BYTES)),
                album: Some("\\".repeat(MAX_TRACK_ALBUM_BYTES)),
                provenance: Some(provenance.clone()),
                requester_user_id: None,
                duration_ms: Some(180_000),
                position_ms: 0,
                seekable: true,
            })
            .collect();
        snapshot.queued_tracks = u32::try_from(snapshot.upcoming_tracks.len()).expect("queue");
        snapshot.current_track = Some(TrackSnapshot {
            track_id: Uuid::from_u128(u128::MAX),
            title: "\\".repeat(MAX_TRACK_TITLE_BYTES),
            artist: Some("\\".repeat(MAX_TRACK_ARTIST_BYTES)),
            album: Some("\\".repeat(MAX_TRACK_ALBUM_BYTES)),
            provenance: Some(provenance),
            requester_user_id: None,
            duration_ms: Some(180_000),
            position_ms: 180_000,
            seekable: true,
        });

        snapshot.validate_public_shape().expect("bounded snapshot");
        let encoded = serde_json::to_vec(&snapshot).expect("snapshot JSON");
        assert!(encoded.len() < MAX_PLAYER_SNAPSHOT_JSON_BYTES);
    }

    #[test]
    fn public_shape_rejects_unbounded_or_inconsistent_queue_data() {
        let mut snapshot = snapshot();
        snapshot.upcoming_tracks.push(TrackSnapshot {
            track_id: Uuid::from_u128(1),
            title: "valid".into(),
            artist: None,
            album: None,
            provenance: None,
            requester_user_id: None,
            duration_ms: Some(1_000),
            position_ms: 0,
            seekable: true,
        });
        assert_eq!(
            snapshot.validate_public_shape(),
            Err(PlayerSnapshotValidationError::QueueCountMismatch)
        );
        snapshot.queued_tracks = 1;
        snapshot.upcoming_tracks[0].title = "音".repeat(MAX_TRACK_TITLE_BYTES);
        assert_eq!(
            snapshot.validate_public_shape(),
            Err(PlayerSnapshotValidationError::InvalidTitle)
        );
    }

    fn snapshot() -> PlayerSnapshot {
        PlayerSnapshot {
            guild_id: GuildId::new(1).expect("guild"),
            voice_channel_id: None,
            revision: StateRevision::new(1),
            state: PlayerState::IdleConnected,
            current_track: None,
            queued_tracks: 0,
            upcoming_tracks: Vec::new(),
            has_previous_track: false,
            volume: Volume::DEFAULT,
            repeat_mode: RepeatMode::Off,
            shuffle_enabled: false,
            hrir_preset: None,
            spatial_audio_enabled: false,
            observed_at: UnixTimeMillis::new(1),
        }
    }

    fn maximum_public_provenance() -> TrackProvenance {
        let apple = PublicMediaPage::new(
            MediaProvider::AppleMusic,
            format!(
                "https://music.apple.com/jp/album/{}/12345678901234567890?i=12345678901234567890",
                "a".repeat(512)
            ),
        )
        .expect("maximum Apple page");
        let soundcloud = PublicMediaPage::new(
            MediaProvider::SoundCloud,
            format!(
                "https://soundcloud.com/{}/{}",
                "a".repeat(256),
                "b".repeat(256)
            ),
        )
        .expect("maximum SoundCloud page");
        TrackProvenance::new(Some(apple), soundcloud).expect("provenance")
    }
}
