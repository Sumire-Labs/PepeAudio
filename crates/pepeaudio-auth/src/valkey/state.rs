use redis::Script;
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize as _, Zeroizing};

use super::{
    ValkeyAuthStore,
    scripts::{CONSUME_STATE, RESERVE_STATE},
    store::MAX_PENDING_OAUTH_STATES,
};
use crate::{
    PendingOAuth, PendingOAuthStore, RepositoryError, crypto::opaque_hash, ports::BoxAuthFuture,
};

const MAX_PENDING_JSON_BYTES: usize = 1024;

impl PendingOAuthStore for ValkeyAuthStore {
    fn reserve<'a>(
        &'a self,
        state: &'a str,
        pending: PendingOAuth,
    ) -> BoxAuthFuture<'a, Result<(), RepositoryError>> {
        Box::pin(async move {
            validate_state(state)?;
            let wire = PendingWireRef {
                verifier: pending.verifier.as_str(),
                created_at_ms: pending.created_at_ms,
            };
            let encoded =
                Zeroizing::new(serde_json::to_string(&wire).map_err(|_| RepositoryError::Corrupt)?);
            let mut connection = self.connection.clone();
            let state_hash = opaque_hash(state);
            let counter_ttl = self
                .state_ttl_ms
                .checked_mul(2)
                .ok_or(RepositoryError::Corrupt)?;
            let result: i32 = Script::new(RESERVE_STATE)
                .key(self.state_key(&state_hash))
                .key(self.pending_states_key())
                .arg(encoded.as_str())
                .arg(self.state_ttl_ms)
                .arg(counter_ttl)
                .arg(MAX_PENDING_OAUTH_STATES)
                .arg(&state_hash)
                .invoke_async(&mut connection)
                .await
                .map_err(|_| RepositoryError::Unavailable)?;
            match result {
                1 => Ok(()),
                0 => Err(RepositoryError::Collision),
                -1 => Err(RepositoryError::CapacityExceeded),
                _ => Err(RepositoryError::Corrupt),
            }
        })
    }

    fn consume<'a>(
        &'a self,
        state: &'a str,
    ) -> BoxAuthFuture<'a, Result<Option<PendingOAuth>, RepositoryError>> {
        Box::pin(async move {
            validate_state(state)?;
            let mut connection = self.connection.clone();
            let state_hash = opaque_hash(state);
            let encoded: Option<String> = Script::new(CONSUME_STATE)
                .key(self.state_key(&state_hash))
                .key(self.pending_states_key())
                .arg(state_hash)
                .invoke_async(&mut connection)
                .await
                .map_err(|_| RepositoryError::Unavailable)?;
            let Some(mut encoded) = encoded else {
                return Ok(None);
            };
            if encoded.len() > MAX_PENDING_JSON_BYTES {
                encoded.zeroize();
                return Err(RepositoryError::Corrupt);
            }
            let wire =
                serde_json::from_str::<PendingWire>(&encoded).map_err(|_| RepositoryError::Corrupt);
            encoded.zeroize();
            let mut wire = wire?;
            if wire.verifier.len() != 43 || !is_base64url(&wire.verifier) {
                wire.verifier.zeroize();
                return Err(RepositoryError::Corrupt);
            }
            Ok(Some(PendingOAuth {
                verifier: Zeroizing::new(wire.verifier),
                created_at_ms: wire.created_at_ms,
            }))
        })
    }
}

fn validate_state(state: &str) -> Result<(), RepositoryError> {
    if state.len() == 43 && is_base64url(state) {
        Ok(())
    } else {
        Err(RepositoryError::Corrupt)
    }
}

fn is_base64url(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

#[derive(Serialize)]
struct PendingWireRef<'a> {
    verifier: &'a str,
    created_at_ms: u64,
}

#[derive(Deserialize)]
struct PendingWire {
    verifier: String,
    created_at_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::validate_state;
    use crate::crypto::random_token;

    #[test]
    fn state_has_exact_random_token_shape() {
        assert!(validate_state(&random_token().expect("random state")).is_ok());
        assert!(validate_state("short").is_err());
        assert!(validate_state("aBCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_+abcd").is_err());
    }
}
