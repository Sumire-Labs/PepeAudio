use std::time::Duration;

/// Shared admission policy for authenticated Web player commands.
///
/// The actor limit prevents one member from monopolizing controls, while the
/// guild limit bounds accepted command-result and idempotency records even if
/// many members submit commands concurrently.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlayerCommandRateLimit {
    per_actor_per_guild: u32,
    per_guild: u32,
    window: Duration,
}

impl PlayerCommandRateLimit {
    pub const STANDARD: Self = Self {
        per_actor_per_guild: 20,
        per_guild: 60,
        window: Duration::from_mins(1),
    };

    #[must_use]
    pub const fn per_actor_per_guild(self) -> u32 {
        self.per_actor_per_guild
    }

    #[must_use]
    pub const fn per_guild(self) -> u32 {
        self.per_guild
    }

    /// Fixed-window duration. It also bounds the public `Retry-After` value.
    #[must_use]
    pub const fn window(self) -> Duration {
        self.window
    }
}

impl Default for PlayerCommandRateLimit {
    fn default() -> Self {
        Self::STANDARD
    }
}
