use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use p256::{
    ecdsa::{Signature, SigningKey, signature::Signer as _},
    pkcs8::DecodePrivateKey,
};
use serde::Serialize;
use tokio::sync::Mutex;

use crate::{CatalogError, CatalogProvider, CatalogResult, secret::Secret};

const TOKEN_LIFETIME: Duration = Duration::from_hours(1);
const EARLY_REFRESH: Duration = Duration::from_mins(5);

#[derive(Serialize)]
struct Header<'a> {
    alg: &'static str,
    kid: &'a str,
    typ: &'static str,
}

#[derive(Serialize)]
struct Claims<'a> {
    iss: &'a str,
    iat: u64,
    exp: u64,
}

struct CachedToken {
    value: Secret,
    refresh_at_epoch_seconds: u64,
}

pub(super) struct DeveloperToken {
    team_id: String,
    key_id: String,
    signing_key: SigningKey,
    cached: Mutex<Option<CachedToken>>,
}

impl DeveloperToken {
    pub(super) fn new(
        team_id: impl Into<String>,
        key_id: impl Into<String>,
        private_key_pem: &str,
    ) -> CatalogResult<Self> {
        let team_id = team_id.into();
        let key_id = key_id.into();
        if !valid_apple_id(&team_id)
            || !valid_apple_id(&key_id)
            || private_key_pem.len() > 16 * 1024
        {
            return Err(CatalogError::InvalidCredentials(
                CatalogProvider::AppleMusic,
            ));
        }
        let signing_key = SigningKey::from_pkcs8_pem(private_key_pem)
            .map_err(|_| CatalogError::InvalidCredentials(CatalogProvider::AppleMusic))?;
        Ok(Self {
            team_id,
            key_id,
            signing_key,
            cached: Mutex::new(None),
        })
    }

    pub(super) async fn token(&self) -> CatalogResult<String> {
        let now = epoch_seconds()?;
        let mut cached = self.cached.lock().await;
        if let Some(token) = cached.as_ref()
            && now < token.refresh_at_epoch_seconds
        {
            return Ok(token.value.expose().to_owned());
        }
        let token = self.sign_at(now)?;
        let refresh_at_epoch_seconds = now
            .checked_add(TOKEN_LIFETIME.saturating_sub(EARLY_REFRESH).as_secs())
            .ok_or(CatalogError::InvalidCredentials(
                CatalogProvider::AppleMusic,
            ))?;
        *cached = Some(CachedToken {
            value: Secret::new(token.clone()),
            refresh_at_epoch_seconds,
        });
        Ok(token)
    }

    fn sign_at(&self, issued_at: u64) -> CatalogResult<String> {
        let expires_at = issued_at.checked_add(TOKEN_LIFETIME.as_secs()).ok_or(
            CatalogError::InvalidCredentials(CatalogProvider::AppleMusic),
        )?;
        let header = encode_json(&Header {
            alg: "ES256",
            kid: &self.key_id,
            typ: "JWT",
        })?;
        let claims = encode_json(&Claims {
            iss: &self.team_id,
            iat: issued_at,
            exp: expires_at,
        })?;
        let signing_input = format!("{header}.{claims}");
        let signature: Signature = self.signing_key.sign(signing_input.as_bytes());
        Ok(format!(
            "{signing_input}.{}",
            URL_SAFE_NO_PAD.encode(signature.to_bytes())
        ))
    }
}

fn encode_json(value: &impl Serialize) -> CatalogResult<String> {
    serde_json::to_vec(value)
        .map(|bytes| URL_SAFE_NO_PAD.encode(bytes))
        .map_err(|_| CatalogError::InvalidCredentials(CatalogProvider::AppleMusic))
}

fn epoch_seconds() -> CatalogResult<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|_| CatalogError::InvalidCredentials(CatalogProvider::AppleMusic))
}

fn valid_apple_id(value: &str) -> bool {
    value.len() == 10 && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use p256::pkcs8::{EncodePrivateKey, LineEnding};

    use super::*;

    fn test_private_key() -> String {
        SigningKey::from_slice(&[7; 32])
            .expect("test signing key")
            .to_pkcs8_pem(LineEnding::LF)
            .expect("PKCS#8 PEM")
            .to_string()
    }

    #[test]
    fn creates_es256_token_with_bounded_expiry() {
        let signer = DeveloperToken::new("ABCDEFGHIJ", "1234567890", &test_private_key())
            .expect("developer token signer");
        let token = signer.sign_at(1_700_000_000).expect("signed JWT");
        let parts = token.split('.').collect::<Vec<_>>();
        assert_eq!(parts.len(), 3);
        let header: serde_json::Value =
            serde_json::from_slice(&URL_SAFE_NO_PAD.decode(parts[0]).expect("header base64"))
                .expect("header JSON");
        let claims: serde_json::Value =
            serde_json::from_slice(&URL_SAFE_NO_PAD.decode(parts[1]).expect("claims base64"))
                .expect("claims JSON");
        assert_eq!(header["alg"], "ES256");
        assert_eq!(header["kid"], "1234567890");
        assert_eq!(claims["iss"], "ABCDEFGHIJ");
        assert_eq!(claims["exp"], 1_700_003_600_u64);
        assert_eq!(
            URL_SAFE_NO_PAD.decode(parts[2]).expect("signature").len(),
            64
        );
    }

    #[test]
    fn rejects_non_pkcs8_key_without_echoing_it() {
        let error = DeveloperToken::new("ABCDEFGHIJ", "1234567890", "private-secret")
            .err()
            .expect("invalid key");
        assert_eq!(
            error,
            CatalogError::InvalidCredentials(CatalogProvider::AppleMusic)
        );
        assert!(!error.to_string().contains("private-secret"));
    }
}
