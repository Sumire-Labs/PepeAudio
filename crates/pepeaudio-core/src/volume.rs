use std::fmt;

use serde::{Deserialize, Serialize};

/// Keeping this value integral makes commands deterministic across JSON, Rust,
/// and JavaScript boundaries. Audio workers convert it to a gain at the edge.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Volume(u8);

impl Volume {
    pub const MUTED: Self = Self(0);
    pub const MAX: Self = Self(100);
    pub const DEFAULT: Self = Self(75);

    /// Accepts an inclusive percentage from 0 through 100.
    ///
    /// # Errors
    ///
    /// Returns [`VolumeError`] for percentages above 100.
    pub const fn new(percent: u8) -> Result<Self, VolumeError> {
        if percent <= Self::MAX.0 {
            Ok(Self(percent))
        } else {
            Err(VolumeError { percent })
        }
    }

    #[must_use]
    pub const fn percent(self) -> u8 {
        self.0
    }

    /// Returns the linear gain expected by the audio worker.
    #[must_use]
    pub fn linear_gain(self) -> f32 {
        f32::from(self.0) / 100.0
    }
}

impl Default for Volume {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl TryFrom<u8> for Volume {
    type Error = VolumeError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<Volume> for u8 {
    fn from(value: Volume) -> Self {
        value.percent()
    }
}

impl Serialize for Volume {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u8(self.0)
    }
}

impl<'de> Deserialize<'de> for Volume {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = u8::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VolumeError {
    percent: u8,
}

impl VolumeError {
    #[must_use]
    pub const fn percent(self) -> u8 {
        self.percent
    }
}

impl fmt::Display for VolumeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "volume must be between 0 and 100 percent, got {}",
            self.percent
        )
    }
}

impl std::error::Error for VolumeError {}

#[cfg(test)]
mod tests {
    use super::{Volume, VolumeError};

    #[test]
    fn accepts_inclusive_bounds() {
        assert_eq!(Volume::new(0), Ok(Volume::MUTED));
        assert_eq!(Volume::new(100), Ok(Volume::MAX));
        assert_eq!(Volume::DEFAULT.percent(), 75);
        assert!((Volume::new(25).expect("valid volume").linear_gain() - 0.25).abs() < f32::EPSILON);
    }

    #[test]
    fn rejects_values_above_one_hundred() {
        assert_eq!(Volume::new(101), Err(VolumeError { percent: 101 }));
        assert!(serde_json::from_str::<Volume>("255").is_err());
    }
}
