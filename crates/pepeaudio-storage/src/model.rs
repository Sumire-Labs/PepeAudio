use std::time::Duration;

use pepeaudio_core::{GuildId, HrirPresetId, UserId, Volume};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

/// Monotonic optimistic-concurrency version for durable guild settings.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct SettingsRevision(u64);

impl SettingsRevision {
    pub const INITIAL: Self = Self(0);

    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Who may mutate ordinary player controls for a guild.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlPolicy {
    /// Members connected to the same voice channel may control playback.
    SameVoiceChannel,
    /// Only a configured DJ role or guild managers may control playback.
    DjOnly,
    /// Only members with Discord's Manage Guild permission may control playback.
    ManageGuild,
}

impl ControlPolicy {
    pub(crate) const fn as_db(self) -> &'static str {
        match self {
            Self::SameVoiceChannel => "same_voice_channel",
            Self::DjOnly => "dj_only",
            Self::ManageGuild => "manage_guild",
        }
    }

    pub(crate) fn from_db(value: &str) -> Option<Self> {
        match value {
            "same_voice_channel" => Some(Self::SameVoiceChannel),
            "dj_only" => Some(Self::DjOnly),
            "manage_guild" => Some(Self::ManageGuild),
            _ => None,
        }
    }
}

/// Durable per-guild defaults. Live track and queue state do not belong here.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuildSettings {
    pub guild_id: GuildId,
    /// Initial and persistent guild volume.
    pub volume: Volume,
    pub idle_disconnect: Duration,
    /// Authorization policy shared by Discord and web controls.
    pub control_policy: ControlPolicy,
    pub dj_role_id: Option<u64>,
    pub default_hrir_preset_id: Option<HrirPresetId>,
    /// Whether new players start with spatial processing enabled.
    pub spatial_audio_enabled: bool,
    /// Optimistic-concurrency revision.
    pub revision: SettingsRevision,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

/// `HeSuVi` channel arrangement recorded in a preset WAV.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HrirChannelLayout {
    /// Seven-channel mirrored form.
    Hesuvi7,
    /// Fourteen-channel explicit left/right-ear form.
    Hesuvi14,
}

impl HrirChannelLayout {
    pub(crate) const fn as_db(self) -> &'static str {
        match self {
            Self::Hesuvi7 => "hesuvi_7",
            Self::Hesuvi14 => "hesuvi_14",
        }
    }

    pub(crate) fn from_db(value: &str) -> Option<Self> {
        match value {
            "hesuvi_7" => Some(Self::Hesuvi7),
            "hesuvi_14" => Some(Self::Hesuvi14),
            _ => None,
        }
    }
}

/// Durable metadata for an HRIR asset stored outside `PostgreSQL`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HrirPresetMetadata {
    /// Stable preset identifier used in snapshots and commands.
    pub preset_id: HrirPresetId,
    /// Owning guild, or `None` for an operator-installed global preset.
    pub owner_guild_id: Option<GuildId>,
    pub display_name: String,
    /// Opaque path or object key, never the binary data itself.
    pub storage_key: String,
    /// Lower-case hexadecimal SHA-256 of the immutable source file.
    pub sha256_hex: String,
    pub sample_rate: u32,
    pub channel_layout: HrirChannelLayout,
    pub file_size_bytes: u64,
    /// SPDX identifier or human-readable license name when known.
    pub license_name: Option<String>,
    /// Canonical license or source URL when known.
    pub license_url: Option<String>,
    pub attribution: Option<String>,
    pub created_at: OffsetDateTime,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaylistVisibility {
    Private,
    Guild,
}

impl PlaylistVisibility {
    pub(crate) const fn as_db(self) -> &'static str {
        match self {
            Self::Private => "private",
            Self::Guild => "guild",
        }
    }

    pub(crate) fn from_db(value: &str) -> Option<Self> {
        match value {
            "private" => Some(Self::Private),
            "guild" => Some(Self::Guild),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Playlist {
    pub playlist_id: Uuid,
    pub guild_id: GuildId,
    /// Discord user who owns private access and edit rights.
    pub owner_user_id: UserId,
    pub name: String,
    pub description: Option<String>,
    pub visibility: PlaylistVisibility,
    /// Optimistic-concurrency revision.
    pub revision: SettingsRevision,
    pub created_at: OffsetDateTime,
    /// Last header or track-list update.
    pub updated_at: OffsetDateTime,
}

/// Kind of stable source reference stored in a playlist.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TrackSourceKind {
    /// Direct HTTP(S) media URL subject to revalidation at playback time.
    DirectUrl,
    /// Discord attachment copied to operator-controlled storage.
    ManagedUpload,
}

impl TrackSourceKind {
    pub(crate) const fn as_db(self) -> &'static str {
        match self {
            Self::DirectUrl => "direct_url",
            Self::ManagedUpload => "managed_upload",
        }
    }

    pub(crate) fn from_db(value: &str) -> Option<Self> {
        match value {
            "direct_url" => Some(Self::DirectUrl),
            "managed_upload" => Some(Self::ManagedUpload),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlaylistTrack {
    pub track_id: Uuid,
    /// Zero-based position within the playlist.
    pub position: u32,
    pub source_kind: TrackSourceKind,
    /// URL or opaque managed-object reference.
    pub source_reference: String,
    /// Cached display title; playback resolution may refresh it.
    pub title: String,
    /// Cached duration when known.
    pub duration_ms: Option<u64>,
    pub added_by_user_id: UserId,
    pub created_at: OffsetDateTime,
}
