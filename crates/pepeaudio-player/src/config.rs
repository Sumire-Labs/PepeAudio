use std::{fmt, num::NonZeroUsize, time::Duration};

use pepeaudio_core::MAX_SNAPSHOT_QUEUE_ITEMS;

pub const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_mins(5);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlayerConfig {
    pub(crate) command_capacity: NonZeroUsize,
    pub(crate) event_capacity: NonZeroUsize,
    pub(crate) max_queued_tracks: NonZeroUsize,
    pub(crate) idle_timeout: Duration,
}

impl PlayerConfig {
    /// # Errors
    ///
    /// Returns [`PlayerConfigError`] when a capacity or timeout is zero.
    pub fn new(
        command_capacity: usize,
        event_capacity: usize,
        max_queued_tracks: usize,
        idle_timeout: Duration,
    ) -> Result<Self, PlayerConfigError> {
        if max_queued_tracks > MAX_SNAPSHOT_QUEUE_ITEMS {
            return Err(PlayerConfigError::CapacityTooLarge {
                name: "max_queued_tracks",
                maximum: MAX_SNAPSHOT_QUEUE_ITEMS,
            });
        }
        Ok(Self {
            command_capacity: non_zero("command_capacity", command_capacity)?,
            event_capacity: non_zero("event_capacity", event_capacity)?,
            max_queued_tracks: non_zero("max_queued_tracks", max_queued_tracks)?,
            idle_timeout: if idle_timeout.is_zero() {
                return Err(PlayerConfigError::ZeroIdleTimeout);
            } else {
                idle_timeout
            },
        })
    }

    #[must_use]
    pub const fn command_capacity(self) -> usize {
        self.command_capacity.get()
    }

    #[must_use]
    pub const fn event_capacity(self) -> usize {
        self.event_capacity.get()
    }

    /// Maximum number of upcoming tracks, excluding the active track.
    #[must_use]
    pub const fn max_queued_tracks(self) -> usize {
        self.max_queued_tracks.get()
    }

    #[must_use]
    pub const fn idle_timeout(self) -> Duration {
        self.idle_timeout
    }
}

impl Default for PlayerConfig {
    fn default() -> Self {
        Self::new(64, 128, 100, DEFAULT_IDLE_TIMEOUT).expect("default limits are non-zero")
    }
}

fn non_zero(name: &'static str, value: usize) -> Result<NonZeroUsize, PlayerConfigError> {
    NonZeroUsize::new(value).ok_or(PlayerConfigError::ZeroCapacity { name })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlayerConfigError {
    ZeroCapacity { name: &'static str },
    CapacityTooLarge { name: &'static str, maximum: usize },
    ZeroIdleTimeout,
}

impl fmt::Display for PlayerConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroCapacity { name } => write!(formatter, "{name} must be greater than zero"),
            Self::CapacityTooLarge { name, maximum } => {
                write!(formatter, "{name} must not exceed {maximum}")
            }
            Self::ZeroIdleTimeout => formatter.write_str("idle_timeout must be greater than zero"),
        }
    }
}

impl std::error::Error for PlayerConfigError {}

#[cfg(test)]
mod tests {
    use pepeaudio_core::MAX_SNAPSHOT_QUEUE_ITEMS;

    use super::{DEFAULT_IDLE_TIMEOUT, PlayerConfig, PlayerConfigError};

    #[test]
    fn queue_capacity_cannot_exceed_the_public_snapshot_contract() {
        assert!(PlayerConfig::new(1, 1, MAX_SNAPSHOT_QUEUE_ITEMS, DEFAULT_IDLE_TIMEOUT).is_ok());
        assert_eq!(
            PlayerConfig::new(1, 1, MAX_SNAPSHOT_QUEUE_ITEMS + 1, DEFAULT_IDLE_TIMEOUT),
            Err(PlayerConfigError::CapacityTooLarge {
                name: "max_queued_tracks",
                maximum: MAX_SNAPSHOT_QUEUE_ITEMS,
            })
        );
    }
}
