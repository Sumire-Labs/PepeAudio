use std::{fmt, sync::Arc};

use axum::{
    extract::FromRequestParts,
    http::{HeaderMap, header::COOKIE, request::Parts},
};
use pepeaudio_core::UserId;
use thiserror::Error;

use crate::{ApiError, AppState, BoxPortFuture};

mod session_fingerprint;

pub use session_fingerprint::SessionFingerprint;

pub const DEV_USER_HEADER: &str = "x-pepeaudio-dev-user-id";
pub const CSRF_HEADER: &str = "x-csrf-token";
pub const SESSION_COOKIE: &str = "__Host-pepeaudio_session";

/// Authenticated request identity and its session-bound CSRF expectation.
///
/// Production constructs this only after validating an opaque server-side
/// session. It must not trust a browser-sent user ID or expose OAuth
/// credentials to JavaScript.
#[derive(Clone)]
pub struct Principal {
    user_id: UserId,
    csrf_token: Arc<str>,
    session_fingerprint: Option<SessionFingerprint>,
}

impl Principal {
    /// The caller is responsible for generating and protecting a high-entropy
    /// session-bound CSRF value.
    ///
    /// # Errors
    ///
    /// Returns an error for an obviously weak CSRF value. This length check is
    /// only a configuration guard and does not replace cryptographic random
    /// generation in a production session adapter.
    pub fn from_session(
        user_id: UserId,
        csrf_token: impl Into<Arc<str>>,
        session_fingerprint: SessionFingerprint,
    ) -> Result<Self, PrincipalConfigError> {
        let csrf_token = csrf_token.into();
        if csrf_token.len() < 16 {
            return Err(PrincipalConfigError::WeakCsrfToken);
        }
        Ok(Self {
            user_id,
            csrf_token,
            session_fingerprint: Some(session_fingerprint),
        })
    }

    #[must_use]
    pub const fn user_id(&self) -> UserId {
        self.user_id
    }

    pub(crate) fn csrf_token(&self) -> &str {
        &self.csrf_token
    }

    /// Server-only fingerprint of the opaque session that established this
    /// principal. Development-header identities deliberately return `None`.
    #[must_use]
    pub const fn session_fingerprint(&self) -> Option<&SessionFingerprint> {
        self.session_fingerprint.as_ref()
    }
}

impl fmt::Debug for Principal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Principal")
            .field("user_id", &self.user_id)
            .field("csrf_token", &"[redacted]")
            .field("session_fingerprint", &self.session_fingerprint)
            .finish()
    }
}

/// Adapter that resolves a request into a server-authenticated principal.
pub trait PrincipalAuthenticator: Send + Sync {
    /// Authenticates request headers without returning raw session credentials.
    fn authenticate<'a>(
        &'a self,
        headers: &'a HeaderMap,
    ) -> BoxPortFuture<'a, Result<Principal, AuthenticationError>>;
}

/// Production cookie authenticator backed by an opaque server-side session
/// store. The browser never receives Discord OAuth access or refresh tokens.
#[derive(Clone)]
pub struct SessionAuthenticator<S> {
    sessions: S,
}

impl<S> SessionAuthenticator<S> {
    #[must_use]
    pub const fn new(sessions: S) -> Self {
        Self { sessions }
    }
}

impl<S> PrincipalAuthenticator for SessionAuthenticator<S>
where
    S: SessionStore,
{
    fn authenticate<'a>(
        &'a self,
        headers: &'a HeaderMap,
    ) -> BoxPortFuture<'a, Result<Principal, AuthenticationError>> {
        Box::pin(async move {
            let token = session_cookie(headers).ok_or(AuthenticationError::Unauthenticated)?;
            let session = self.sessions.load_session(token).await?;
            Principal::from_session(
                session.user_id,
                session.csrf_token,
                session.session_fingerprint,
            )
            .map_err(|_| AuthenticationError::Unavailable)
        })
    }
}

/// Authenticated data held only by the server-side session store.
#[derive(Clone)]
pub struct SessionRecord {
    pub user_id: UserId,
    /// Session-bound synchronizer token required for mutations.
    pub csrf_token: Arc<str>,
    /// Fingerprint of the exact opaque session that was loaded.
    pub session_fingerprint: SessionFingerprint,
}

impl fmt::Debug for SessionRecord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SessionRecord")
            .field("user_id", &self.user_id)
            .field("csrf_token", &"[redacted]")
            .field("session_fingerprint", &self.session_fingerprint)
            .finish()
    }
}

/// Opaque web-session lookup port.
pub trait SessionStore: Send + Sync + 'static {
    /// Resolves a high-entropy opaque cookie value into server-side state.
    fn load_session<'a>(
        &'a self,
        opaque_token: &'a str,
    ) -> BoxPortFuture<'a, Result<SessionRecord, AuthenticationError>>;
}

fn session_cookie(headers: &HeaderMap) -> Option<&str> {
    headers
        .get_all(COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|line| line.split(';'))
        .find_map(|pair| {
            let (name, value) = pair.trim().split_once('=')?;
            (name == SESSION_COOKIE
                && !value.is_empty()
                && value.len() <= 256
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')))
            .then_some(value)
        })
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderMap, HeaderValue, header::COOKIE};

    use super::session_cookie;

    #[test]
    fn extracts_only_canonical_opaque_session_cookie() {
        let mut headers = HeaderMap::new();
        headers.insert(
            COOKIE,
            HeaderValue::from_static("theme=dark; __Host-pepeaudio_session=abc_DEF-123"),
        );
        assert_eq!(session_cookie(&headers), Some("abc_DEF-123"));

        headers.insert(
            COOKIE,
            HeaderValue::from_static("__Host-pepeaudio_session=bad%2Ftoken"),
        );
        assert_eq!(session_cookie(&headers), None);
    }
}

/// Explicit development/test authenticator for one configured user.
///
/// This is not OAuth, does not use a secure session cookie, and must never be
/// enabled by a production deployment. It accepts only the exact user ID
/// configured at startup and supplies a separately configured CSRF expectation.
#[derive(Clone)]
pub struct DevHeaderAuthenticator {
    user_id: UserId,
    csrf_token: Arc<str>,
}

impl DevHeaderAuthenticator {
    /// # Errors
    ///
    /// Returns an error when the development CSRF value is too short to avoid
    /// accidental empty/default configuration.
    pub fn new(
        user_id: UserId,
        csrf_token: impl Into<Arc<str>>,
    ) -> Result<Self, DevAuthConfigError> {
        let csrf_token = csrf_token.into();
        if csrf_token.len() < 16 {
            return Err(DevAuthConfigError::WeakCsrfToken);
        }
        Ok(Self {
            user_id,
            csrf_token,
        })
    }
}

impl fmt::Debug for DevHeaderAuthenticator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DevHeaderAuthenticator")
            .field("user_id", &self.user_id)
            .field("csrf_token", &"[redacted]")
            .finish()
    }
}

impl PrincipalAuthenticator for DevHeaderAuthenticator {
    fn authenticate<'a>(
        &'a self,
        headers: &'a HeaderMap,
    ) -> BoxPortFuture<'a, Result<Principal, AuthenticationError>> {
        Box::pin(async move {
            let supplied = headers
                .get(DEV_USER_HEADER)
                .ok_or(AuthenticationError::Unauthenticated)?
                .to_str()
                .map_err(|_| AuthenticationError::Unauthenticated)?
                .parse::<UserId>()
                .map_err(|_| AuthenticationError::Unauthenticated)?;
            if supplied != self.user_id {
                return Err(AuthenticationError::Unauthenticated);
            }
            Ok(Principal {
                user_id: self.user_id,
                csrf_token: Arc::clone(&self.csrf_token),
                session_fingerprint: None,
            })
        })
    }
}

impl FromRequestParts<AppState> for Principal {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        state
            .authenticator
            .authenticate(&parts.headers)
            .await
            .map_err(ApiError::from)
    }
}

/// Authentication failure safe for HTTP mapping.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum AuthenticationError {
    #[error("authentication required")]
    Unauthenticated,
    #[error("authentication service unavailable")]
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum PrincipalConfigError {
    #[error("session CSRF token must contain at least 16 bytes")]
    WeakCsrfToken,
    #[error("session fingerprint must be a canonical SHA-256 base64url digest")]
    InvalidSessionFingerprint,
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum DevAuthConfigError {
    #[error("development CSRF token must contain at least 16 bytes")]
    WeakCsrfToken,
}
