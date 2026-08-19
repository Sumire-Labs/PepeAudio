use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, Mac};
use pepeaudio_core::{GuildId, StateRevision};
use sha2::Sha256;
use thiserror::Error;

const PREFIX: &str = "pa1";
const TAG_BYTES: usize = 10;

/// Action encoded into a Discord component `custom_id`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComponentAction {
    PlayPause,
    Previous,
    Skip,
    Stop,
    Repeat,
    Shuffle,
    Spatial,
    Volume,
    Hrir,
}

impl ComponentAction {
    const fn code(self) -> &'static str {
        match self {
            Self::PlayPause => "p",
            Self::Previous => "b",
            Self::Skip => "n",
            Self::Stop => "x",
            Self::Repeat => "r",
            Self::Shuffle => "s",
            Self::Spatial => "a",
            Self::Volume => "v",
            Self::Hrir => "h",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "p" => Self::PlayPause,
            "b" => Self::Previous,
            "n" => Self::Skip,
            "x" => Self::Stop,
            "r" => Self::Repeat,
            "s" => Self::Shuffle,
            "a" => Self::Spatial,
            "v" => Self::Volume,
            "h" => Self::Hrir,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodedComponentId {
    pub action: ComponentAction,
    pub guild_id: GuildId,
    pub revision: StateRevision,
}

#[derive(Clone)]
pub struct ComponentIdCodec {
    key: Vec<u8>,
}

impl ComponentIdCodec {
    /// # Errors
    ///
    /// Returns [`ComponentIdError::WeakKey`] if the key is shorter than 32 bytes.
    pub fn new(key: impl AsRef<[u8]>) -> Result<Self, ComponentIdError> {
        let key = key.as_ref();
        if key.len() < 32 {
            return Err(ComponentIdError::WeakKey);
        }
        Ok(Self { key: key.to_vec() })
    }

    /// Encodes and authenticates an ID under Discord's 100-character limit.
    #[must_use]
    pub fn encode(
        &self,
        action: ComponentAction,
        guild_id: GuildId,
        revision: StateRevision,
    ) -> String {
        let body = format!(
            "{PREFIX}.{}.{:x}.{:x}",
            action.code(),
            guild_id.get(),
            revision.get()
        );
        let signature = self.tag(body.as_bytes());
        format!("{body}.{}", URL_SAFE_NO_PAD.encode(signature))
    }

    /// Parses an ID and verifies its constant-time HMAC tag.
    ///
    /// # Errors
    ///
    /// Returns [`ComponentIdError`] when validation fails.
    pub fn decode(&self, value: &str) -> Result<DecodedComponentId, ComponentIdError> {
        if value.len() > 100 {
            return Err(ComponentIdError::TooLong);
        }
        let mut parts = value.rsplitn(2, '.');
        let tag = parts.next().ok_or(ComponentIdError::Malformed)?;
        let body = parts.next().ok_or(ComponentIdError::Malformed)?;
        let decoded_tag = URL_SAFE_NO_PAD
            .decode(tag)
            .map_err(|_| ComponentIdError::Malformed)?;
        if decoded_tag.len() != TAG_BYTES {
            return Err(ComponentIdError::Malformed);
        }
        let mut verifier =
            Hmac::<Sha256>::new_from_slice(&self.key).map_err(|_| ComponentIdError::Malformed)?;
        verifier.update(body.as_bytes());
        verifier
            .verify_truncated_left(&decoded_tag)
            .map_err(|_| ComponentIdError::BadSignature)?;

        let fields: Vec<_> = body.split('.').collect();
        let [prefix, action, guild, revision] = fields.as_slice() else {
            return Err(ComponentIdError::Malformed);
        };
        if *prefix != PREFIX {
            return Err(ComponentIdError::UnsupportedVersion);
        }
        let action = ComponentAction::parse(action).ok_or(ComponentIdError::UnknownAction)?;
        let guild = u64::from_str_radix(guild, 16).map_err(|_| ComponentIdError::Malformed)?;
        let guild_id = GuildId::new(guild).map_err(|_| ComponentIdError::Malformed)?;
        let revision =
            u64::from_str_radix(revision, 16).map_err(|_| ComponentIdError::Malformed)?;
        Ok(DecodedComponentId {
            action,
            guild_id,
            revision: StateRevision::new(revision),
        })
    }

    fn tag(&self, value: &[u8]) -> [u8; TAG_BYTES] {
        let mut mac = Hmac::<Sha256>::new_from_slice(&self.key).expect("HMAC accepts any key");
        mac.update(value);
        let full = mac.finalize().into_bytes();
        let mut result = [0; TAG_BYTES];
        result.copy_from_slice(&full[..TAG_BYTES]);
        result
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ComponentIdError {
    #[error("component signing keys must contain at least 32 bytes")]
    WeakKey,
    #[error("component custom ID exceeds 100 bytes")]
    TooLong,
    #[error("component custom ID is malformed")]
    Malformed,
    #[error("component custom ID uses an unsupported version")]
    UnsupportedVersion,
    #[error("component custom ID contains an unknown action")]
    UnknownAction,
    #[error("component custom ID signature is invalid")]
    BadSignature,
}

#[cfg(test)]
mod tests {
    use pepeaudio_core::{GuildId, StateRevision};

    use super::{ComponentAction, ComponentIdCodec, ComponentIdError, DecodedComponentId};

    fn codec() -> ComponentIdCodec {
        ComponentIdCodec::new([7; 32]).expect("strong key")
    }

    #[test]
    fn round_trip_is_short_and_authenticated() {
        let expected = DecodedComponentId {
            action: ComponentAction::Skip,
            guild_id: GuildId::new(u64::MAX).expect("non-zero"),
            revision: StateRevision::new(u64::MAX),
        };
        let encoded = codec().encode(expected.action, expected.guild_id, expected.revision);
        assert!(encoded.len() < 100);
        assert_eq!(codec().decode(&encoded), Ok(expected));
    }

    #[test]
    fn tampering_is_rejected() {
        let encoded = codec().encode(
            ComponentAction::Stop,
            GuildId::new(1).expect("non-zero"),
            StateRevision::new(2),
        );
        let tampered = encoded.replacen(".x.", ".n.", 1);
        assert_eq!(
            codec().decode(&tampered),
            Err(ComponentIdError::BadSignature)
        );
        assert_eq!(
            codec().decode("pa1.x.1.2."),
            Err(ComponentIdError::Malformed)
        );
    }
}
