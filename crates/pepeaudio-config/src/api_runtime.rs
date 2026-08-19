use std::{
    net::SocketAddr,
    num::{NonZeroU32, NonZeroU64},
    time::Duration,
};

use url::Url;

use crate::{
    ConfigResult, ConfigSource, Environment, SecretString,
    load::{bounded, required, secret, sensitive_required},
    validate::{connection_url, http_url, keyspace, nonzero_u64, socket_addr},
};

const MAX_SESSION_SECONDS: u64 = 30 * 60;

/// Configuration required only by the HTTP/OAuth process.
///
/// The Discord Bot token, component-signing key, media tools, and shard range
/// are deliberately absent from this API-only model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApiRuntimeConfig {
    pub database_url: SecretString,
    pub valkey_url: SecretString,
    /// Exact browser origin used for same-origin mutation checks.
    pub public_base_url: Url,
    pub api_bind: SocketAddr,
    pub discord_client_id: NonZeroU64,
    pub discord_client_secret: SecretString,
    /// Exact HTTPS callback registered in the Discord Developer Portal.
    pub discord_oauth_redirect_url: Url,
    pub valkey_keyspace: String,
    pub shard_total: NonZeroU32,
    /// Fixed local redirect after successful login.
    pub auth_success_path: String,
    /// Hard server-side session lifetime.
    pub session_absolute_ttl: Duration,
    /// Sliding inactive-session lifetime.
    pub session_idle_ttl: Duration,
    /// One-time OAuth state lifetime.
    pub oauth_state_ttl: Duration,
}

impl ApiRuntimeConfig {
    /// # Errors
    ///
    /// Returns [`crate::ConfigError`] when a required API setting is missing or invalid.
    pub fn from_env() -> ConfigResult<Self> {
        Self::from_source(&Environment)
    }

    /// # Errors
    ///
    /// Returns [`crate::ConfigError`] when a required API setting is missing or invalid.
    pub fn from_source(source: &impl ConfigSource) -> ConfigResult<Self> {
        let database_url = connection_secret(
            source,
            "PEPEAUDIO_DATABASE_URL",
            "PEPEAUDIO_DATABASE_URL_FILE",
            &["postgres", "postgresql"],
            true,
        )?;
        let valkey_url = connection_secret(
            source,
            "PEPEAUDIO_VALKEY_URL",
            "PEPEAUDIO_VALKEY_URL_FILE",
            &["redis", "rediss"],
            false,
        )?;
        let public_base_url = http_url(
            "PEPEAUDIO_PUBLIC_BASE_URL",
            &required(source, "PEPEAUDIO_PUBLIC_BASE_URL")?,
        )?;
        let callback = http_url(
            "PEPEAUDIO_DISCORD_OAUTH_REDIRECT_URL",
            &required(source, "PEPEAUDIO_DISCORD_OAUTH_REDIRECT_URL")?,
        )?;
        let session_absolute_ttl = seconds(
            source,
            "PEPEAUDIO_SESSION_ABSOLUTE_SECONDS",
            60,
            MAX_SESSION_SECONDS,
        )?;
        let session_idle_ttl = seconds(
            source,
            "PEPEAUDIO_SESSION_IDLE_SECONDS",
            60,
            MAX_SESSION_SECONDS,
        )?;
        if session_idle_ttl > session_absolute_ttl {
            return Err(crate::ConfigError::Invalid {
                name: "PEPEAUDIO_SESSION_IDLE_SECONDS",
                reason: "must not exceed PEPEAUDIO_SESSION_ABSOLUTE_SECONDS",
            });
        }

        Ok(Self {
            database_url,
            valkey_url,
            public_base_url,
            api_bind: socket_addr(
                "PEPEAUDIO_API_BIND",
                &required(source, "PEPEAUDIO_API_BIND")?,
            )?,
            discord_client_id: nonzero_u64(
                "PEPEAUDIO_DISCORD_CLIENT_ID",
                &required(source, "PEPEAUDIO_DISCORD_CLIENT_ID")?,
            )?,
            discord_client_secret: secret(
                source,
                "PEPEAUDIO_DISCORD_CLIENT_SECRET",
                "PEPEAUDIO_DISCORD_CLIENT_SECRET_FILE",
                16,
            )?,
            discord_oauth_redirect_url: callback,
            valkey_keyspace: keyspace(
                "PEPEAUDIO_VALKEY_KEYSPACE",
                required(source, "PEPEAUDIO_VALKEY_KEYSPACE")?,
            )?,
            shard_total: crate::load::parse(source, "PEPEAUDIO_SHARD_TOTAL")?,
            auth_success_path: success_path(required(source, "PEPEAUDIO_AUTH_SUCCESS_PATH")?)?,
            session_absolute_ttl,
            session_idle_ttl,
            oauth_state_ttl: seconds(source, "PEPEAUDIO_OAUTH_STATE_SECONDS", 30, 10 * 60)?,
        })
    }
}

fn connection_secret(
    source: &impl ConfigSource,
    name: &'static str,
    file_name: &'static str,
    schemes: &[&str],
    require_database: bool,
) -> ConfigResult<SecretString> {
    let value = sensitive_required(source, name, file_name)?;
    connection_url(name, &value, schemes, require_database)?;
    Ok(SecretString::new(value))
}

fn seconds(
    source: &impl ConfigSource,
    name: &'static str,
    minimum: u64,
    maximum: u64,
) -> ConfigResult<Duration> {
    Ok(Duration::from_secs(bounded(
        source, name, minimum, maximum,
    )?))
}

fn success_path(value: String) -> ConfigResult<String> {
    if value.starts_with('/')
        && !value.starts_with("//")
        && !value.contains(['\r', '\n', '\\', '?', '#'])
    {
        Ok(value)
    } else {
        Err(crate::ConfigError::Invalid {
            name: "PEPEAUDIO_AUTH_SUCCESS_PATH",
            reason: "must be one fixed local absolute path",
        })
    }
}
