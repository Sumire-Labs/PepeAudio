use std::{ffi::OsString, path::Path};

use pepeaudio_config::{BotRuntimeConfig, ConfigSource, Environment};
use thiserror::Error;

pub use pepeaudio_config::{ShardConfig, ShardConfigError};

const DISCORD_TOKEN: &str = "PEPEAUDIO_DISCORD_TOKEN";
const DISCORD_TOKEN_FILE: &str = "PEPEAUDIO_DISCORD_TOKEN_FILE";
const COMPONENT_SIGNING_KEY: &str = "PEPEAUDIO_COMPONENT_SIGNING_KEY";
const COMPONENT_SIGNING_KEY_FILE: &str = "PEPEAUDIO_COMPONENT_SIGNING_KEY_FILE";

/// Process configuration sourced from `PEPEAUDIO_*` environment variables.
#[derive(Clone)]
pub struct BotConfig {
    pub discord_token: String,
    pub component_signing_key: Vec<u8>,
    /// Optional guild used for fast development command registration.
    pub development_guild_id: Option<u64>,
    pub runtime: BotRuntimeConfig,
}

impl BotConfig {
    /// # Errors
    ///
    /// Returns [`ConfigError`] when a required setting is absent or invalid.
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_source(&Environment)
    }

    /// # Errors
    ///
    /// Returns [`ConfigError`] when a required setting is absent or invalid.
    pub fn from_source(source: &impl ConfigSource) -> Result<Self, ConfigError> {
        let runtime = BotRuntimeConfig::from_source(source)?;
        let discord_token = secret_from_source(source, DISCORD_TOKEN, DISCORD_TOKEN_FILE)?;
        let component_signing_key =
            secret_from_source(source, COMPONENT_SIGNING_KEY, COMPONENT_SIGNING_KEY_FILE)?
                .into_bytes();
        if component_signing_key.len() < 32 {
            return Err(ConfigError::SigningKeyTooShort);
        }
        let development_guild_id = optional_u64(source, "PEPEAUDIO_DEVELOPMENT_GUILD_ID")?;
        Ok(Self {
            discord_token,
            component_signing_key,
            development_guild_id,
            runtime,
        })
    }

    #[must_use]
    pub const fn shards(&self) -> &ShardConfig {
        &self.runtime.shards
    }
}

fn secret_from_source(
    source: &impl ConfigSource,
    name: &'static str,
    file_name: &'static str,
) -> Result<String, ConfigError> {
    resolve_secret(
        name,
        file_name,
        source.get(name)?.map(OsString::from),
        source.get(file_name)?.map(OsString::from),
        |path| std::fs::read(path),
    )
}

fn resolve_secret(
    name: &'static str,
    file_name: &'static str,
    direct: Option<OsString>,
    file: Option<OsString>,
    read: impl FnOnce(&Path) -> std::io::Result<Vec<u8>>,
) -> Result<String, ConfigError> {
    let value = match (direct, file) {
        (Some(_), Some(_)) => {
            return Err(ConfigError::ConflictingSecretSources { name, file_name });
        }
        (Some(value), None) => value
            .into_string()
            .map_err(|_| ConfigError::InvalidUnicode { name })?,
        (None, Some(path)) => {
            if path.is_empty() {
                return Err(ConfigError::SecretFile { name: file_name });
            }
            let bytes =
                read(Path::new(&path)).map_err(|_| ConfigError::SecretFile { name: file_name })?;
            let value = String::from_utf8(bytes)
                .map_err(|_| ConfigError::SecretFile { name: file_name })?;
            remove_file_line_ending(value)
        }
        (None, None) => return Err(ConfigError::MissingSecret { name, file_name }),
    };

    if value.is_empty()
        || value.chars().all(char::is_whitespace)
        || value.contains(['\r', '\n'])
        || is_known_placeholder(&value)
    {
        return Err(ConfigError::InvalidSecret { name });
    }
    Ok(value)
}

fn is_known_placeholder(value: &str) -> bool {
    value == "replace-me" || value.starts_with("replace-with-")
}

fn remove_file_line_ending(mut value: String) -> String {
    if value.ends_with("\r\n") {
        value.truncate(value.len() - 2);
    } else if value.ends_with('\n') {
        value.pop();
    }
    value
}

fn optional_u64(
    source: &impl ConfigSource,
    name: &'static str,
) -> Result<Option<u64>, ConfigError> {
    match source.get(name)? {
        Some(value) => value
            .parse()
            .map(Some)
            .map_err(|_| ConfigError::InvalidInteger { name }),
        None => Ok(None),
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error(transparent)]
    Runtime(#[from] pepeaudio_config::ConfigError),
    #[error("required secret {name} or {file_name} is missing")]
    MissingSecret {
        name: &'static str,
        file_name: &'static str,
    },
    #[error("secret variables {name} and {file_name} cannot both be set")]
    ConflictingSecretSources {
        name: &'static str,
        file_name: &'static str,
    },
    #[error("secret file configured by {name} could not be read")]
    SecretFile { name: &'static str },
    #[error("secret configured by {name} is invalid")]
    InvalidSecret { name: &'static str },
    #[error("environment variable {name} is not a valid integer")]
    InvalidInteger { name: &'static str },
    #[error("environment variable {name} is not valid Unicode")]
    InvalidUnicode { name: &'static str },
    #[error("component signing key must contain at least 32 bytes")]
    SigningKeyTooShort,
}

#[cfg(test)]
mod tests {
    use std::{ffi::OsString, io};

    use pepeaudio_config::MapSource;

    use super::{
        COMPONENT_SIGNING_KEY, COMPONENT_SIGNING_KEY_FILE, ConfigError, DISCORD_TOKEN,
        DISCORD_TOKEN_FILE, ShardConfig, ShardConfigError, resolve_secret,
    };

    fn bot_source() -> MapSource {
        MapSource::new()
            .with(DISCORD_TOKEN, "discord-token-value-that-is-long-enough")
            .with(
                COMPONENT_SIGNING_KEY,
                "component-signing-key-that-is-long-enough",
            )
            .with("PEPEAUDIO_SHARD_TOTAL", "4")
            .with("PEPEAUDIO_SHARD_START", "0")
            .with("PEPEAUDIO_INSTANCE_ID", "bot-0")
            .with("PEPEAUDIO_DATABASE_URL", "postgres://app:secret@db/app")
            .with("PEPEAUDIO_VALKEY_URL", "redis://default:secret@valkey/0")
            .with("PEPEAUDIO_VALKEY_KEYSPACE", "pepeaudio-test")
            .with("PEPEAUDIO_IDLE_DISCONNECT_SECONDS", "300")
            .with("PEPEAUDIO_DEFAULT_VOLUME_PERCENT", "75")
            .with("PEPEAUDIO_MAX_QUEUE_ITEMS", "100")
            .with("PEPEAUDIO_MAX_TRACK_DURATION_SECONDS", "21600")
            .with("PEPEAUDIO_MAX_UPLOAD_BYTES", "10485760")
            .with("PEPEAUDIO_MAX_MANAGED_MEDIA_BYTES", "10737418240")
            .with("PEPEAUDIO_FFMPEG_PATH", "ffmpeg")
            .with("PEPEAUDIO_FFPROBE_PATH", "ffprobe")
            .with("PEPEAUDIO_HRIR_DIRECTORY", "/app/hrir")
            .with("PEPEAUDIO_UPLOAD_DIRECTORY", "/app/uploads")
            .with("PEPEAUDIO_ENABLE_SITE_EXTRACTORS", "false")
            .with("PEPEAUDIO_YTDLP_PATH", "yt-dlp")
    }

    #[test]
    fn process_config_uses_the_canonical_runtime_shard_topology() {
        let config = super::BotConfig::from_source(&bot_source()).expect("valid Bot config");

        assert_eq!(config.shards().total(), 4);
        assert_eq!(config.shards().range(), 0..4);
        assert_eq!(config.runtime.instance_id, "bot-0");
    }

    #[test]
    fn shard_range_is_validated() {
        let shards = ShardConfig::new(8, 2, 5).expect("valid range");
        assert_eq!(shards.total(), 8);
        assert_eq!(shards.range(), 2..5);
        assert_eq!(
            ShardConfig::new(8, 2, 9),
            Err(ShardConfigError::OutOfRange {
                end_exclusive: 9,
                total: 8,
            })
        );
        assert!(matches!(
            ShardConfig::new(8, 2, 2),
            Err(ShardConfigError::EmptyOrInvertedRange { .. })
        ));
    }

    #[test]
    fn direct_and_file_secret_sources_conflict_without_exposure() {
        let value = "do-not-render-this-token";
        let path = "C:\\private\\do-not-render-this-path";
        let error = resolve_secret(
            DISCORD_TOKEN,
            DISCORD_TOKEN_FILE,
            Some(OsString::from(value)),
            Some(OsString::from(path)),
            |_| panic!("conflict must be rejected before file access"),
        )
        .expect_err("conflicting sources are invalid");

        assert!(matches!(
            error,
            ConfigError::ConflictingSecretSources { .. }
        ));
        let rendered = format!("{error:?} {error}");
        assert!(!rendered.contains(value));
        assert!(!rendered.contains(path));
    }

    #[test]
    fn secret_files_remove_one_lf_or_crlf() {
        for (bytes, expected) in [
            (b"discord-token\n".as_slice(), "discord-token"),
            (
                b"component-signing-key\r\n".as_slice(),
                "component-signing-key",
            ),
        ] {
            let actual = resolve_secret(
                COMPONENT_SIGNING_KEY,
                COMPONENT_SIGNING_KEY_FILE,
                None,
                Some(OsString::from("opaque-path")),
                |_| Ok(bytes.to_vec()),
            )
            .expect("file secret is valid");
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn secret_file_failures_never_include_the_path() {
        let path = "C:\\private\\sensitive-location";
        let error = resolve_secret(
            DISCORD_TOKEN,
            DISCORD_TOKEN_FILE,
            None,
            Some(OsString::from(path)),
            |_| Err(io::Error::new(io::ErrorKind::PermissionDenied, path)),
        )
        .expect_err("read failure is reported");

        let rendered = format!("{error:?} {error}");
        assert!(!rendered.contains(path));
    }

    #[test]
    fn published_secret_placeholders_fail_closed() {
        let packaged_example = include_str!("../../../secrets/component_signing_key.txt.example");
        for placeholder in [
            "replace-me",
            "replace-with-at-least-32-random-bytes",
            packaged_example.trim(),
        ] {
            let error = resolve_secret(
                COMPONENT_SIGNING_KEY,
                COMPONENT_SIGNING_KEY_FILE,
                Some(OsString::from(placeholder)),
                None,
                |_| unreachable!("a direct secret never reads a file"),
            )
            .expect_err("known placeholders must never become signing keys");
            assert!(matches!(error, ConfigError::InvalidSecret { .. }));
            assert!(!format!("{error:?} {error}").contains(placeholder));
        }
    }
}
