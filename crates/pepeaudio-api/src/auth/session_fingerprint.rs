use std::{fmt, sync::Arc};

use super::PrincipalConfigError;

/// SHA-256 fingerprint of an opaque session cookie, encoded as unpadded
/// base64url. This server-only identity is safe to retain after the raw cookie
/// value has left the authentication boundary.
#[derive(Clone, Eq, PartialEq)]
pub struct SessionFingerprint(Arc<str>);

impl SessionFingerprint {
    /// # Errors
    ///
    /// Returns an error unless `encoded` is a 32-byte digest encoded as 43
    /// unpadded base64url characters.
    pub fn new(encoded: impl Into<Arc<str>>) -> Result<Self, PrincipalConfigError> {
        let encoded = encoded.into();
        if encoded.len() != 43
            || !encoded
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
            || !has_canonical_final_sextet(&encoded)
        {
            return Err(PrincipalConfigError::InvalidSessionFingerprint);
        }
        Ok(Self(encoded))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SessionFingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SessionFingerprint([redacted])")
    }
}

fn has_canonical_final_sextet(encoded: &str) -> bool {
    matches!(
        encoded.as_bytes().last(),
        Some(
            b'A' | b'E'
                | b'I'
                | b'M'
                | b'Q'
                | b'U'
                | b'Y'
                | b'c'
                | b'g'
                | b'k'
                | b'o'
                | b's'
                | b'w'
                | b'0'
                | b'4'
                | b'8'
        )
    )
}
