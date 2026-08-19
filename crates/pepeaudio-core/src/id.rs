use std::{fmt, num::NonZeroU64, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SnowflakeParseError {
    Zero,
    InvalidDecimal,
}

impl fmt::Display for SnowflakeParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Zero => formatter.write_str("a Discord snowflake must be non-zero"),
            Self::InvalidDecimal => {
                formatter.write_str("a Discord snowflake must be an unsigned 64-bit decimal string")
            }
        }
    }
}

impl std::error::Error for SnowflakeParseError {}

macro_rules! snowflake_id {
    ($name:ident, $description:literal) => {
        #[doc = $description]
        ///
        /// Its serialized representation is a decimal string so JavaScript
        /// clients never lose precision.
        #[repr(transparent)]
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(NonZeroU64);

        impl $name {
            /// # Errors
            ///
            /// Returns [`SnowflakeParseError::Zero`] when `value` is zero.
            pub const fn new(value: u64) -> Result<Self, SnowflakeParseError> {
                match NonZeroU64::new(value) {
                    Some(value) => Ok(Self(value)),
                    None => Err(SnowflakeParseError::Zero),
                }
            }

            #[must_use]
            pub const fn get(self) -> u64 {
                self.0.get()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }

        impl FromStr for $name {
            type Err = SnowflakeParseError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                let value = value
                    .parse::<u64>()
                    .map_err(|_| SnowflakeParseError::InvalidDecimal)?;
                Self::new(value)
            }
        }

        impl TryFrom<u64> for $name {
            type Error = SnowflakeParseError;

            fn try_from(value: u64) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        impl From<$name> for u64 {
            fn from(value: $name) -> Self {
                value.get()
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.collect_str(self)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                value.parse().map_err(D::Error::custom)
            }
        }
    };
}

snowflake_id!(GuildId, "A validated Discord guild snowflake.");
snowflake_id!(ChannelId, "A validated Discord channel snowflake.");
snowflake_id!(UserId, "A validated Discord user snowflake.");

#[cfg(test)]
mod tests {
    use super::{GuildId, SnowflakeParseError};

    #[test]
    fn snowflakes_round_trip_as_json_strings() {
        let id = GuildId::new(123_456_789_012_345_678).expect("non-zero snowflake");
        let encoded = serde_json::to_string(&id).expect("serialize snowflake");

        assert_eq!(encoded, "\"123456789012345678\"");
        assert_eq!(
            serde_json::from_str::<GuildId>(&encoded).expect("deserialize snowflake"),
            id
        );
    }

    #[test]
    fn snowflakes_reject_zero_and_json_numbers() {
        assert_eq!(GuildId::new(0), Err(SnowflakeParseError::Zero));
        assert!(serde_json::from_str::<GuildId>("42").is_err());
        assert!(serde_json::from_str::<GuildId>("\"not-a-number\"").is_err());
    }
}
