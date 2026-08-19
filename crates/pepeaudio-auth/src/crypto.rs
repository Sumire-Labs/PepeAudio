use std::time::{SystemTime, UNIX_EPOCH};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use sha2::{Digest as _, Sha256};
use subtle::ConstantTimeEq as _;
use thiserror::Error;
use zeroize::Zeroizing;

pub(crate) const RANDOM_TOKEN_BYTES: usize = 32;

#[derive(Clone)]
pub(crate) struct OAuthMaterial {
    pub state: String,
    pub verifier: Zeroizing<String>,
    pub challenge: String,
}

pub(crate) fn oauth_material() -> Result<OAuthMaterial, CryptoError> {
    let state = random_token()?;
    let verifier = Zeroizing::new(random_token()?);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    Ok(OAuthMaterial {
        state,
        verifier,
        challenge,
    })
}

pub(crate) fn random_token() -> Result<String, CryptoError> {
    let mut bytes = Zeroizing::new([0_u8; RANDOM_TOKEN_BYTES]);
    getrandom::fill(bytes.as_mut()).map_err(|_| CryptoError::Unavailable)?;
    Ok(URL_SAFE_NO_PAD.encode(bytes.as_ref()))
}

pub(crate) fn opaque_hash(token: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(token.as_bytes()))
}

pub(crate) fn constant_time_eq(left: &str, right: &str) -> bool {
    left.as_bytes().ct_eq(right.as_bytes()).into()
}

pub(crate) fn unix_millis() -> Result<u64, CryptoError> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| CryptoError::Clock)?
        .as_millis();
    u64::try_from(millis).map_err(|_| CryptoError::Clock)
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub(crate) enum CryptoError {
    #[error("operating system random source unavailable")]
    Unavailable,
    #[error("system clock unavailable")]
    Clock,
}

#[cfg(test)]
mod tests {
    use super::{RANDOM_TOKEN_BYTES, constant_time_eq, oauth_material, opaque_hash, random_token};

    #[test]
    fn tokens_are_url_safe_and_have_expected_entropy_size() {
        let first = random_token().expect("random token");
        let second = random_token().expect("random token");
        assert_ne!(first, second);
        assert_eq!(first.len(), (RANDOM_TOKEN_BYTES * 4).div_ceil(3));
        assert!(
            first
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        );
    }

    #[test]
    fn pkce_uses_distinct_verifier_and_s256_challenge() {
        let material = oauth_material().expect("OAuth material");
        assert_ne!(material.verifier.as_str(), material.challenge);
        assert_eq!(material.verifier.len(), 43);
        assert_eq!(material.challenge.len(), 43);
    }

    #[test]
    fn comparison_and_hash_are_stable() {
        assert!(constant_time_eq("same", "same"));
        assert!(!constant_time_eq("same", "different"));
        assert_eq!(opaque_hash("session"), opaque_hash("session"));
    }
}
