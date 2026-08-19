use pepeaudio_core::{GuildId, UserId};

use crate::{StorageError, StorageResult};

/// Validated prefix used to isolate environments in one Valkey deployment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Keyspace(String);

impl Keyspace {
    /// Validates an environment prefix such as `pepeaudio:dev`.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty, oversized, or non-ASCII prefix.
    pub fn new(prefix: impl Into<String>) -> StorageResult<Self> {
        let prefix = prefix.into();
        let valid = !prefix.is_empty()
            && prefix.len() <= 64
            && prefix
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'-' | b'_'));
        if valid {
            Ok(Self(prefix))
        } else {
            Err(StorageError::InvalidIdentifier {
                kind: "Valkey keyspace",
                reason: "must be 1-64 ASCII letters, digits, colons, underscores, or hyphens",
            })
        }
    }

    pub(super) fn snapshot(&self, guild_id: GuildId) -> String {
        format!("{}:player:{guild_id}:snapshot", self.0)
    }

    pub(super) fn snapshot_revision(&self, guild_id: GuildId) -> String {
        format!("{}:player:{guild_id}:snapshot:revision", self.0)
    }

    pub(super) fn snapshot_event(&self, guild_id: GuildId) -> String {
        format!("{}:evt:guild:{guild_id}", self.0)
    }

    pub(super) fn snapshot_event_pattern(&self) -> String {
        format!("{}:evt:guild:*", self.0)
    }

    pub(super) fn command_stream(&self, shard_id: u32) -> String {
        format!("{}:cmd:shard:{shard_id}", self.0)
    }

    pub(super) fn dedupe(&self, guild_id: GuildId, key: uuid::Uuid) -> String {
        format!("{}:processed:{guild_id}:{key}", self.0)
    }

    pub(super) fn command_result(&self, guild_id: GuildId, command_id: uuid::Uuid) -> String {
        format!("{}:cmd-result:{guild_id}:{command_id}", self.0)
    }

    pub(super) fn player_command_actor_rate_limit(
        &self,
        guild_id: GuildId,
        actor_user_id: UserId,
    ) -> String {
        format!(
            "{}:rate:player-command:guild:{guild_id}:actor:{actor_user_id}",
            self.0
        )
    }

    pub(super) fn player_command_guild_rate_limit(&self, guild_id: GuildId) -> String {
        format!("{}:rate:player-command:guild:{guild_id}:all", self.0)
    }

    pub(super) fn bot_presence(&self, guild_id: GuildId) -> String {
        format!("{}:bot-presence:{guild_id}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use pepeaudio_core::{GuildId, UserId};
    use uuid::Uuid;

    use super::Keyspace;

    #[test]
    fn builds_stable_keys_without_user_supplied_fragments() {
        let keys = Keyspace::new("pepeaudio:test").expect("valid prefix");
        let guild = GuildId::new(42).expect("guild");

        assert_eq!(keys.snapshot(guild), "pepeaudio:test:player:42:snapshot");
        assert_eq!(
            keys.snapshot_revision(guild),
            "pepeaudio:test:player:42:snapshot:revision"
        );
        assert_eq!(keys.bot_presence(guild), "pepeaudio:test:bot-presence:42");
        assert_eq!(keys.command_stream(3), "pepeaudio:test:cmd:shard:3");
        assert_eq!(
            keys.dedupe(guild, Uuid::nil()),
            "pepeaudio:test:processed:42:00000000-0000-0000-0000-000000000000"
        );
        assert_eq!(
            keys.command_result(guild, Uuid::nil()),
            "pepeaudio:test:cmd-result:42:00000000-0000-0000-0000-000000000000"
        );
        assert_eq!(
            keys.player_command_actor_rate_limit(guild, UserId::new(7).expect("actor user ID")),
            "pepeaudio:test:rate:player-command:guild:42:actor:7"
        );
        assert_eq!(
            keys.player_command_guild_rate_limit(guild),
            "pepeaudio:test:rate:player-command:guild:42:all"
        );
    }

    #[test]
    fn rejects_prefixes_that_can_corrupt_key_structure() {
        assert!(Keyspace::new("").is_err());
        assert!(Keyspace::new("space is invalid").is_err());
        assert!(Keyspace::new("slash/is/invalid").is_err());
    }
}
