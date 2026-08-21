use std::{
    net::SocketAddr,
    num::{NonZeroU32, NonZeroU64},
    ops::Range,
    path::PathBuf,
    time::Duration,
};

use pepeaudio_core::{HrirPresetId, Volume};
use url::Url;

use crate::{
    CatalogConfig, ConfigResult, ConfigSource, Environment, SecretString, ShardConfigError,
    load::{load, load_bot_runtime},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppConfig {
    pub discord: DiscordConfig,
    pub shards: ShardConfig,
    pub instance_id: String,
    pub services: ServiceConfig,
    pub player: PlayerLimits,
    pub tools: ToolConfig,
    pub catalog: CatalogConfig,
}

impl AppConfig {
    /// Secret values are never included in validation errors.
    ///
    /// # Errors
    ///
    /// Returns [`crate::ConfigError`] when a required value is missing or invalid.
    pub fn from_env() -> ConfigResult<Self> {
        Self::from_source(&Environment)
    }

    /// # Errors
    ///
    /// Returns [`crate::ConfigError`] when a required value is missing or invalid.
    pub fn from_source(source: &impl ConfigSource) -> ConfigResult<Self> {
        load(source)
    }
}

/// Configuration required only by a Discord shard process.
///
/// OAuth client credentials and browser-session keys are deliberately absent,
/// so Bot containers do not need access to API-only secrets.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BotRuntimeConfig {
    pub instance_id: String,
    pub shards: ShardConfig,
    pub database_url: SecretString,
    pub valkey_url: SecretString,
    pub valkey_keyspace: String,
    pub player: PlayerLimits,
    pub tools: ToolConfig,
    pub catalog: CatalogConfig,
}

impl BotRuntimeConfig {
    /// # Errors
    ///
    /// Returns [`crate::ConfigError`] when a required Bot setting is missing or invalid.
    pub fn from_env() -> ConfigResult<Self> {
        Self::from_source(&Environment)
    }

    /// # Errors
    ///
    /// Returns [`crate::ConfigError`] when a required Bot setting is missing or invalid.
    pub fn from_source(source: &impl ConfigSource) -> ConfigResult<Self> {
        load_bot_runtime(source)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscordConfig {
    pub token: SecretString,
    pub application_id: NonZeroU64,
    pub client_id: NonZeroU64,
    pub client_secret: SecretString,
    pub oauth_redirect_url: Url,
}

/// Fixed shard topology and the half-open range owned by one process.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShardConfig {
    pub total: NonZeroU32,
    pub start: u32,
    pub end_exclusive: u32,
}

impl ShardConfig {
    /// # Errors
    ///
    /// Returns [`ShardConfigError`] for a zero total or invalid owned range.
    pub fn new(total: u32, start: u32, end_exclusive: u32) -> Result<Self, ShardConfigError> {
        let total = NonZeroU32::new(total).ok_or(ShardConfigError::ZeroTotal)?;
        if start >= end_exclusive {
            return Err(ShardConfigError::EmptyOrInvertedRange {
                start,
                end_exclusive,
            });
        }
        if end_exclusive > total.get() {
            return Err(ShardConfigError::OutOfRange {
                end_exclusive,
                total: total.get(),
            });
        }
        Ok(Self {
            total,
            start,
            end_exclusive,
        })
    }

    #[must_use]
    pub const fn total(&self) -> u32 {
        self.total.get()
    }

    #[must_use]
    pub const fn range(&self) -> Range<u32> {
        self.start..self.end_exclusive
    }

    #[must_use]
    pub const fn owns(&self, shard_id: u32) -> bool {
        self.start <= shard_id && shard_id < self.end_exclusive
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceConfig {
    pub database_url: SecretString,
    pub valkey_url: SecretString,
    /// Browser-visible origin used for redirects and generated links.
    pub public_base_url: Url,
    pub api_bind: SocketAddr,
    pub session_cookie_secure: bool,
    pub session_key: SecretString,
    pub valkey_keyspace: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlayerLimits {
    pub idle_disconnect: Duration,
    pub default_volume: Volume,
    pub default_hrir_preset: HrirPresetId,
    pub default_spatial_audio_enabled: bool,
    pub max_queue_items: NonZeroU32,
    pub max_track_duration: Duration,
    pub max_upload_bytes: NonZeroU64,
    /// Per-track byte cap for resolved `YouTube` and `SoundCloud` media.
    pub max_site_media_bytes: NonZeroU64,
    /// Effective playlist import cap, never greater than the queue bound.
    pub max_playlist_items: NonZeroU32,
    /// Hard process-local budget for all managed media files and reservations.
    pub max_managed_media_bytes: NonZeroU64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolConfig {
    pub ffmpeg_path: PathBuf,
    pub ffprobe_path: PathBuf,
    /// Read-only directory containing imported HRIR files.
    pub hrir_directory: PathBuf,
    /// Private writable directory for temporary media uploads.
    pub upload_directory: PathBuf,
    pub site_extractors_enabled: bool,
    pub ytdlp_path: PathBuf,
    pub deno_path: PathBuf,
    pub deno_directory: PathBuf,
}
