use std::fmt;

use serde::{Deserialize, Serialize};

/// Maximum accepted length of a stable HRIR preset identifier, in UTF-8 bytes.
pub const MAX_HRIR_PRESET_ID_BYTES: usize = 128;

/// A stable identifier of an installed HRIR preset.
#[repr(transparent)]
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct HrirPresetId(String);

impl HrirPresetId {
    /// Identifiers must be non-empty, have no surrounding whitespace or
    /// control characters, and fit within [`MAX_HRIR_PRESET_ID_BYTES`].
    ///
    /// # Errors
    ///
    /// Returns [`HrirPresetIdError`] when these requirements are not met.
    pub fn new(value: impl Into<String>) -> Result<Self, HrirPresetIdError> {
        let value = value.into();

        if value.is_empty() {
            return Err(HrirPresetIdError::Empty);
        }
        if value.len() > MAX_HRIR_PRESET_ID_BYTES {
            return Err(HrirPresetIdError::TooLong { bytes: value.len() });
        }
        if value.trim() != value {
            return Err(HrirPresetIdError::SurroundingWhitespace);
        }
        if value.chars().any(char::is_control) {
            return Err(HrirPresetIdError::ControlCharacter);
        }

        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for HrirPresetId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for HrirPresetId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HrirPresetIdError {
    Empty,
    TooLong { bytes: usize },
    SurroundingWhitespace,
    ControlCharacter,
}

impl fmt::Display for HrirPresetIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("an HRIR preset identifier cannot be empty"),
            Self::TooLong { bytes } => write!(
                formatter,
                "an HRIR preset identifier cannot exceed {MAX_HRIR_PRESET_ID_BYTES} bytes; got {bytes}"
            ),
            Self::SurroundingWhitespace => {
                formatter.write_str("an HRIR preset identifier cannot have surrounding whitespace")
            }
            Self::ControlCharacter => {
                formatter.write_str("an HRIR preset identifier cannot contain control characters")
            }
        }
    }
}

impl std::error::Error for HrirPresetIdError {}
