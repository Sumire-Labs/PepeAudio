use std::{fmt, time::Duration};

use thiserror::Error;
use url::Url;
use zeroize::Zeroizing;

const MAX_SESSION_ABSOLUTE_TTL: Duration = Duration::from_mins(30);
const DEFAULT_SESSION_IDLE_TTL: Duration = Duration::from_mins(30);
const DEFAULT_OAUTH_STATE_TTL: Duration = Duration::from_mins(5);

/// Secret string which zeroizes its allocation and never implements `Debug`.
#[derive(Clone)]
pub struct SecretString(Zeroizing<String>);

impl SecretString {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(Zeroizing::new(value.into()))
    }

    pub(crate) fn expose(&self) -> &str {
        self.0.as_str()
    }
}

/// Discord OAuth client settings registered in the Developer Portal.
#[derive(Clone)]
pub struct DiscordOAuthConfig {
    pub(crate) client_id: String,
    pub(crate) client_secret: SecretString,
    pub(crate) redirect_uri: Url,
}

impl DiscordOAuthConfig {
    /// # Errors
    ///
    /// Returns an error unless the ID is a snowflake, the secret is non-trivial,
    /// and the callback is an absolute HTTPS URL without query or fragment.
    pub fn new(
        client_id: impl Into<String>,
        client_secret: SecretString,
        redirect_uri: Url,
    ) -> Result<Self, AuthConfigError> {
        let client_id = client_id.into();
        validate_snowflake(&client_id)?;
        if client_secret.expose().len() < 16 {
            return Err(AuthConfigError::WeakClientSecret);
        }
        validate_redirect_uri(&redirect_uri)?;
        Ok(Self {
            client_id,
            client_secret,
            redirect_uri,
        })
    }

    /// Exact callback URI sent to both Discord authorization and token exchange.
    #[must_use]
    pub fn redirect_uri(&self) -> &Url {
        &self.redirect_uri
    }
}

impl fmt::Debug for DiscordOAuthConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DiscordOAuthConfig")
            .field("client_id", &self.client_id)
            .field("client_secret", &"[REDACTED]")
            .field("redirect_uri", &self.redirect_uri)
            .finish()
    }
}

/// Absolute and idle server-side session limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(clippy::struct_field_names)]
pub struct SessionPolicy {
    /// Hard lifetime which sliding refresh may never exceed.
    pub(crate) absolute_ttl: Duration,
    /// Sliding inactivity lifetime.
    pub(crate) idle_ttl: Duration,
    /// One-time OAuth state lifetime.
    pub(crate) oauth_state_ttl: Duration,
}

impl SessionPolicy {
    /// # Errors
    ///
    /// Rejects zero durations or an idle lifetime longer than the absolute one.
    pub fn new(
        absolute_ttl: Duration,
        idle_ttl: Duration,
        oauth_state_ttl: Duration,
    ) -> Result<Self, AuthConfigError> {
        if absolute_ttl.is_zero() || idle_ttl.is_zero() || oauth_state_ttl.is_zero() {
            return Err(AuthConfigError::ZeroDuration);
        }
        if idle_ttl > absolute_ttl {
            return Err(AuthConfigError::IdleExceedsAbsolute);
        }
        if absolute_ttl > MAX_SESSION_ABSOLUTE_TTL {
            return Err(AuthConfigError::AbsoluteLifetimeTooLong);
        }
        if oauth_state_ttl > Duration::from_mins(10) {
            return Err(AuthConfigError::StateLifetimeTooLong);
        }
        Ok(Self {
            absolute_ttl,
            idle_ttl,
            oauth_state_ttl,
        })
    }

    #[must_use]
    pub const fn absolute_ttl(self) -> Duration {
        self.absolute_ttl
    }

    #[must_use]
    pub const fn idle_ttl(self) -> Duration {
        self.idle_ttl
    }

    #[must_use]
    pub const fn oauth_state_ttl(self) -> Duration {
        self.oauth_state_ttl
    }
}

impl Default for SessionPolicy {
    fn default() -> Self {
        Self {
            absolute_ttl: MAX_SESSION_ABSOLUTE_TTL,
            idle_ttl: DEFAULT_SESSION_IDLE_TTL,
            oauth_state_ttl: DEFAULT_OAUTH_STATE_TTL,
        }
    }
}

#[derive(Clone, Debug)]
pub struct AuthConfig {
    pub(crate) discord: DiscordOAuthConfig,
    pub(crate) session: SessionPolicy,
    pub(crate) keyspace: String,
    pub(crate) success_redirect: String,
}

impl AuthConfig {
    /// `success_redirect` is a fixed local absolute path. No request parameter
    /// can override it, preventing callback open redirects.
    ///
    /// # Errors
    ///
    /// Rejects an unsafe keyspace, redirect path, or invalid session policy.
    pub fn new(
        discord: DiscordOAuthConfig,
        session: SessionPolicy,
        keyspace: impl Into<String>,
        success_redirect: impl Into<String>,
    ) -> Result<Self, AuthConfigError> {
        let keyspace = keyspace.into();
        if keyspace.is_empty()
            || keyspace.len() > 64
            || !keyspace
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'-' | b'_'))
        {
            return Err(AuthConfigError::InvalidKeyspace);
        }
        let success_redirect = success_redirect.into();
        if !success_redirect.starts_with('/')
            || success_redirect.starts_with("//")
            || success_redirect.contains(['\r', '\n', '\\'])
        {
            return Err(AuthConfigError::InvalidSuccessRedirect);
        }
        Ok(Self {
            discord,
            session,
            keyspace,
            success_redirect,
        })
    }

    #[must_use]
    pub const fn session_policy(&self) -> SessionPolicy {
        self.session
    }

    #[must_use]
    pub const fn discord(&self) -> &DiscordOAuthConfig {
        &self.discord
    }
}

fn validate_snowflake(value: &str) -> Result<(), AuthConfigError> {
    if value.len() > 20 || value.parse::<u64>().ok().as_ref().is_none_or(|id| *id == 0) {
        Err(AuthConfigError::InvalidClientId)
    } else {
        Ok(())
    }
}

fn validate_redirect_uri(url: &Url) -> Result<(), AuthConfigError> {
    let valid = url.scheme() == "https"
        && url.host_str().is_some()
        && url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none();
    if valid {
        Ok(())
    } else {
        Err(AuthConfigError::InvalidRedirectUri)
    }
}

/// Fail-closed configuration validation error without secret values.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum AuthConfigError {
    #[error("Discord client ID must be a non-zero decimal snowflake")]
    InvalidClientId,
    #[error("Discord client secret is missing or obviously weak")]
    WeakClientSecret,
    #[error("Discord callback must be an absolute HTTPS URL without query, fragment, or user info")]
    InvalidRedirectUri,
    #[error("session durations must be non-zero")]
    ZeroDuration,
    #[error("session idle lifetime cannot exceed its absolute lifetime")]
    IdleExceedsAbsolute,
    #[error(
        "session absolute lifetime cannot exceed thirty minutes while guild membership is a login-time projection"
    )]
    AbsoluteLifetimeTooLong,
    #[error("OAuth state lifetime cannot exceed ten minutes")]
    StateLifetimeTooLong,
    #[error("Valkey auth keyspace is invalid")]
    InvalidKeyspace,
    #[error("post-login redirect must be a local absolute path")]
    InvalidSuccessRedirect,
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
