use async_trait::async_trait;
use pepeaudio_core::GuildId;
use pepeaudio_storage::{
    ControlPolicy as StoredControlPolicy, GuildSettingsRepository, PostgresStorage,
};
use thiserror::Error;

/// Guild settings needed while authorizing Discord interactions.
#[async_trait]
pub trait GuildPolicyProvider: Send + Sync {
    async fn policy(&self, guild_id: GuildId) -> Result<GuildControlPolicy, GuildPolicyError>;
}

/// Discord-facing authorization facts from durable guild settings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GuildControlPolicy {
    pub control: StoredControlPolicy,
    pub dj_role_id: Option<u64>,
}

impl Default for GuildControlPolicy {
    fn default() -> Self {
        Self {
            control: StoredControlPolicy::SameVoiceChannel,
            dj_role_id: None,
        }
    }
}

impl GuildControlPolicy {
    #[must_use]
    pub fn has_dj_role(self, actor_role_ids: &[u64]) -> bool {
        self.dj_role_id
            .is_some_and(|configured| actor_role_ids.contains(&configured))
    }

    #[must_use]
    pub const fn allows_control(self, manages_guild: bool, has_dj_role: bool) -> bool {
        match self.control {
            StoredControlPolicy::SameVoiceChannel => true,
            StoredControlPolicy::DjOnly => manages_guild || has_dj_role,
            StoredControlPolicy::ManageGuild => manages_guild,
        }
    }
}

/// Policy lookup failure. Storage details are deliberately not exposed to users.
#[derive(Clone, Copy, Debug, Error)]
#[error("the guild control policy is temporarily unavailable")]
pub struct GuildPolicyError;

#[derive(Clone, Copy, Debug, Default)]
pub struct NoopGuildPolicy;

#[async_trait]
impl GuildPolicyProvider for NoopGuildPolicy {
    async fn policy(&self, _guild_id: GuildId) -> Result<GuildControlPolicy, GuildPolicyError> {
        Ok(GuildControlPolicy::default())
    }
}

pub(crate) struct PostgresGuildPolicy {
    postgres: PostgresStorage,
}

impl PostgresGuildPolicy {
    pub(crate) const fn new(postgres: PostgresStorage) -> Self {
        Self { postgres }
    }
}

#[async_trait]
impl GuildPolicyProvider for PostgresGuildPolicy {
    async fn policy(&self, guild_id: GuildId) -> Result<GuildControlPolicy, GuildPolicyError> {
        let settings = self
            .postgres
            .get_guild_settings(guild_id)
            .await
            .map_err(|_| GuildPolicyError)?;
        Ok(
            settings.map_or_else(GuildControlPolicy::default, |item| GuildControlPolicy {
                control: item.control_policy,
                dj_role_id: item.dj_role_id,
            }),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{GuildControlPolicy, GuildPolicyProvider, NoopGuildPolicy};
    use pepeaudio_core::GuildId;
    use pepeaudio_storage::ControlPolicy;

    #[tokio::test]
    async fn noop_policy_never_grants_dj_privileges() {
        let guild = GuildId::new(1).expect("guild id");
        let policy = NoopGuildPolicy.policy(guild).await.expect("lookup");
        assert!(!policy.has_dj_role(&[2, 3]));
        assert!(policy.allows_control(false, false));
    }

    #[test]
    fn control_policy_truth_table_does_not_mix_dj_and_manage_guild() {
        for (control, expected) in [
            (ControlPolicy::SameVoiceChannel, [true, true, true, true]),
            (ControlPolicy::DjOnly, [false, true, true, true]),
            (ControlPolicy::ManageGuild, [false, false, true, true]),
        ] {
            let policy = GuildControlPolicy {
                control,
                dj_role_id: Some(7),
            };
            let actual = [
                policy.allows_control(false, false),
                policy.allows_control(false, true),
                policy.allows_control(true, false),
                policy.allows_control(true, true),
            ];
            assert_eq!(actual, expected, "policy: {control:?}");
        }
    }
}
