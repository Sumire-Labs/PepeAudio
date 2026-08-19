use std::{cmp, sync::Arc};

use pepeaudio_api::{
    AuthenticationError, BoxPortFuture, SessionFingerprint, SessionRecord, SessionStore,
};
use pepeaudio_core::UserId;
use redis::{AsyncCommands as _, Script};
use zeroize::{Zeroize as _, Zeroizing};

use super::{
    ValkeyAuthStore,
    scripts::{CREATE_SESSION, DESTROY_SESSION, LOAD_AND_REFRESH_SESSION},
};
use crate::{
    OpaqueSessionRepository, RepositoryError, SessionData,
    crypto::{opaque_hash, random_token},
    ports::BoxAuthFuture,
};

const MAX_SESSION_JSON_BYTES: usize = 256 * 1024;
const CREATE_ATTEMPTS: usize = 3;

impl OpaqueSessionRepository for ValkeyAuthStore {
    fn create(&self, session: SessionData) -> BoxAuthFuture<'_, Result<String, RepositoryError>> {
        Box::pin(async move {
            validate_session(&session)?;
            if effective_session_expiry(&session, self.absolute_ttl_ms)? != session.expires_at_ms {
                return Err(RepositoryError::Corrupt);
            }
            let now_ms = self
                .clock
                .now_ms()
                .map_err(|_| RepositoryError::Unavailable)?;
            let absolute_remaining = session
                .expires_at_ms
                .checked_sub(now_ms)
                .filter(|remaining| *remaining != 0)
                .ok_or(RepositoryError::Corrupt)?;
            let initial_ttl = cmp::min(self.idle_ttl_ms, absolute_remaining);
            let encoded = Zeroizing::new(
                serde_json::to_string(&session).map_err(|_| RepositoryError::Corrupt)?,
            );

            for _ in 0..CREATE_ATTEMPTS {
                let token = random_token().map_err(|_| RepositoryError::Unavailable)?;
                let hash = opaque_hash(&token);
                let mut connection = self.connection.clone();
                let created: i32 = Script::new(CREATE_SESSION)
                    .key(self.session_key(&hash))
                    .key(self.current_user_key(session.user_id))
                    .arg(encoded.as_str())
                    .arg(initial_ttl)
                    .arg(absolute_remaining)
                    .arg(&hash)
                    .arg(format!("{}:session:", self.keyspace))
                    .invoke_async(&mut connection)
                    .await
                    .map_err(|_| RepositoryError::Unavailable)?;
                if created == 1 {
                    return Ok(token);
                }
            }
            Err(RepositoryError::Collision)
        })
    }

    fn load<'a>(
        &'a self,
        opaque_token: &'a str,
    ) -> BoxAuthFuture<'a, Result<Option<SessionData>, RepositoryError>> {
        Box::pin(async move {
            validate_opaque_token(opaque_token)?;
            let hash = opaque_hash(opaque_token);
            let Some(loaded) = self.load_by_hash(&hash, None).await? else {
                return Ok(None);
            };
            let mut connection = self.connection.clone();
            let pointer: Option<String> = connection
                .get(self.current_user_key(loaded.user_id))
                .await
                .map_err(|_| RepositoryError::Unavailable)?;
            if pointer.as_deref() == Some(hash.as_str()) {
                Ok(Some(loaded))
            } else {
                Ok(None)
            }
        })
    }

    fn load_bound<'a>(
        &'a self,
        user_id: UserId,
        session_fingerprint: &'a str,
    ) -> BoxAuthFuture<'a, Result<Option<SessionData>, RepositoryError>> {
        Box::pin(async move {
            validate_hash(session_fingerprint)?;
            self.load_by_hash(session_fingerprint, Some(user_id)).await
        })
    }

    fn destroy<'a>(
        &'a self,
        opaque_token: &'a str,
    ) -> BoxAuthFuture<'a, Result<(), RepositoryError>> {
        Box::pin(async move {
            validate_opaque_token(opaque_token)?;
            let hash = opaque_hash(opaque_token);
            let Some(session) = self.peek_by_hash(&hash).await? else {
                return Ok(());
            };
            let mut connection = self.connection.clone();
            let pointer: Option<String> = connection
                .get(self.current_user_key(session.user_id))
                .await
                .map_err(|_| RepositoryError::Unavailable)?;
            if pointer.as_deref() != Some(hash.as_str()) {
                return Ok(());
            }
            Script::new(DESTROY_SESSION)
                .key(self.session_key(&hash))
                .key(self.current_user_key(session.user_id))
                .arg(hash)
                .invoke_async::<i32>(&mut connection)
                .await
                .map_err(|_| RepositoryError::Unavailable)?;
            Ok(())
        })
    }
}

impl ValkeyAuthStore {
    async fn load_by_hash(
        &self,
        hash: &str,
        expected_user: Option<UserId>,
    ) -> Result<Option<SessionData>, RepositoryError> {
        let Some(peeked) = self.peek_by_hash(hash).await? else {
            return Ok(None);
        };
        if expected_user.is_some_and(|expected| expected != peeked.user_id) {
            return Err(RepositoryError::Corrupt);
        }
        let now_ms = self
            .clock
            .now_ms()
            .map_err(|_| RepositoryError::Unavailable)?;
        let effective_expiry = effective_session_expiry(&peeked, self.absolute_ttl_ms)?;
        let mut connection = self.connection.clone();
        let encoded: Option<String> = Script::new(LOAD_AND_REFRESH_SESSION)
            .key(self.session_key(hash))
            .key(self.current_user_key(peeked.user_id))
            .arg(hash)
            .arg(now_ms)
            .arg(self.idle_ttl_ms)
            .arg(effective_expiry)
            .invoke_async(&mut connection)
            .await
            .map_err(|_| RepositoryError::Unavailable)?;
        encoded.map(decode_session).transpose()
    }

    async fn peek_by_hash(&self, hash: &str) -> Result<Option<SessionData>, RepositoryError> {
        let mut connection = self.connection.clone();
        let encoded: Option<String> = connection
            .get(self.session_key(hash))
            .await
            .map_err(|_| RepositoryError::Unavailable)?;
        encoded.map(decode_session).transpose()
    }
}

impl SessionStore for ValkeyAuthStore {
    fn load_session<'a>(
        &'a self,
        opaque_token: &'a str,
    ) -> BoxPortFuture<'a, Result<SessionRecord, AuthenticationError>> {
        Box::pin(async move {
            let session = OpaqueSessionRepository::load(self, opaque_token)
                .await
                .map_err(|error| match error {
                    RepositoryError::InvalidToken => AuthenticationError::Unauthenticated,
                    RepositoryError::Collision
                    | RepositoryError::CapacityExceeded
                    | RepositoryError::Unavailable
                    | RepositoryError::Corrupt => AuthenticationError::Unavailable,
                })?
                .ok_or(AuthenticationError::Unauthenticated)?;
            Ok(SessionRecord {
                user_id: session.user_id,
                csrf_token: Arc::from(session.csrf_token),
                session_fingerprint: SessionFingerprint::new(opaque_hash(opaque_token))
                    .map_err(|_| AuthenticationError::Unavailable)?,
            })
        })
    }
}

fn decode_session(mut encoded: String) -> Result<SessionData, RepositoryError> {
    if encoded.len() > MAX_SESSION_JSON_BYTES {
        encoded.zeroize();
        return Err(RepositoryError::Corrupt);
    }
    let parsed = serde_json::from_str(&encoded).map_err(|_| RepositoryError::Corrupt);
    encoded.zeroize();
    let session = parsed?;
    validate_session(&session)?;
    Ok(session)
}

fn validate_session(session: &SessionData) -> Result<(), RepositoryError> {
    if session.is_valid_shape() {
        Ok(())
    } else {
        Err(RepositoryError::Corrupt)
    }
}

fn effective_session_expiry(
    session: &SessionData,
    absolute_ttl_ms: u64,
) -> Result<u64, RepositoryError> {
    let policy_expiry = session
        .created_at_ms
        .checked_add(absolute_ttl_ms)
        .ok_or(RepositoryError::Corrupt)?;
    Ok(cmp::min(session.expires_at_ms, policy_expiry))
}

fn validate_opaque_token(token: &str) -> Result<(), RepositoryError> {
    if token.len() == 43 && is_token_shape(token) {
        Ok(())
    } else {
        Err(RepositoryError::InvalidToken)
    }
}

fn validate_hash(hash: &str) -> Result<(), RepositoryError> {
    validate_opaque_token(hash).map_err(|_| RepositoryError::Corrupt)
}

fn is_token_shape(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

#[cfg(test)]
mod tests {
    use pepeaudio_core::UserId;

    use super::{effective_session_expiry, validate_hash, validate_opaque_token};
    use crate::{
        OAuthProjection, SessionData,
        crypto::{opaque_hash, random_token},
    };

    #[test]
    fn accepts_only_canonical_32_byte_base64url_tokens() {
        let token = random_token().expect("random token");
        assert!(validate_opaque_token(&token).is_ok());
        assert!(validate_hash(&opaque_hash(&token)).is_ok());
        assert!(validate_opaque_token("bad/token").is_err());
    }

    #[test]
    fn current_policy_clamps_sessions_created_by_an_older_release() {
        let created_at_ms = 1_000_000;
        let legacy_lifetime_ms = 7 * 24 * 60 * 60 * 1_000;
        let current_lifetime_ms = 30 * 60 * 1_000;
        let session = SessionData::new(
            OAuthProjection {
                user_id: UserId::new(42).expect("user"),
                profile: None,
                guilds: Vec::new(),
            },
            "a".repeat(43),
            created_at_ms,
            legacy_lifetime_ms,
        )
        .expect("legacy session");

        assert_eq!(
            effective_session_expiry(&session, current_lifetime_ms).expect("effective expiry"),
            created_at_ms + current_lifetime_ms
        );
    }
}
