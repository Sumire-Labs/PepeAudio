use std::{num::NonZeroU32, ops::Range, sync::Arc};

use pepeaudio_core::{CommandEnvelope, PlayerCommand, shard_id};
use pepeaudio_runtime::{CommandAuthorization, CommandAuthorizer};
use pepeaudio_storage::{
    ControlPolicy as StoredControlPolicy, GuildSettingsRepository, PostgresStorage,
};
use serenity::{all::Permissions, cache::Cache};

use crate::voice_facts::current_cached_member;

/// Rechecks every dashboard command using authoritative owning-shard state.
pub(crate) struct DiscordCommandAuthorizer {
    cache: Arc<Cache>,
    postgres: PostgresStorage,
    shard_total: NonZeroU32,
    owned_shards: Range<u32>,
}

impl DiscordCommandAuthorizer {
    pub(crate) fn new(
        cache: Arc<Cache>,
        postgres: PostgresStorage,
        shard_total: NonZeroU32,
        owned_shards: Range<u32>,
    ) -> Self {
        Self {
            cache,
            postgres,
            shard_total,
            owned_shards,
        }
    }
}

#[async_trait::async_trait]
impl CommandAuthorizer for DiscordCommandAuthorizer {
    async fn authorize(&self, envelope: &CommandEnvelope) -> CommandAuthorization {
        if !self
            .owned_shards
            .contains(&shard_id(envelope.guild_id, self.shard_total))
        {
            return CommandAuthorization::Denied;
        }
        let Some(actor) = envelope.actor_user_id else {
            return CommandAuthorization::Denied;
        };
        let facts = match cached_voice_facts(&self.cache, envelope.guild_id.get(), actor.get()) {
            CachedFacts::Available(facts) => facts,
            CachedFacts::Denied => return CommandAuthorization::Denied,
            CachedFacts::Unavailable => return CommandAuthorization::RetryableFailure,
        };

        let Ok(settings) = self.postgres.get_guild_settings(envelope.guild_id).await else {
            return CommandAuthorization::RetryableFailure;
        };
        let manages_guild = facts.permissions.contains(Permissions::MANAGE_GUILD);
        let has_dj_role = settings
            .as_ref()
            .and_then(|item| item.dj_role_id)
            .is_some_and(|role| facts.roles.iter().any(|candidate| candidate.get() == role));
        let privileged = manages_guild || has_dj_role;
        if matches!(
            envelope.command,
            PlayerCommand::Stop | PlayerCommand::Disconnect
        ) {
            return if privileged {
                CommandAuthorization::Allowed
            } else {
                CommandAuthorization::Denied
            };
        }
        let policy = settings.map_or(StoredControlPolicy::SameVoiceChannel, |item| {
            item.control_policy
        });
        match policy {
            StoredControlPolicy::SameVoiceChannel => CommandAuthorization::Allowed,
            StoredControlPolicy::DjOnly if privileged => CommandAuthorization::Allowed,
            StoredControlPolicy::ManageGuild if manages_guild => CommandAuthorization::Allowed,
            StoredControlPolicy::DjOnly | StoredControlPolicy::ManageGuild => {
                CommandAuthorization::Denied
            }
        }
    }
}

struct VoiceFacts {
    permissions: Permissions,
    roles: Vec<serenity::all::RoleId>,
}

enum CachedFacts {
    Available(VoiceFacts),
    Denied,
    Unavailable,
}

fn cached_voice_facts(cache: &Cache, guild_id: u64, actor_id: u64) -> CachedFacts {
    let Some(guild) = cache.guild(serenity::all::GuildId::new(guild_id)) else {
        return CachedFacts::Unavailable;
    };
    let actor_id = serenity::all::UserId::new(actor_id);
    let Some(actor_state) = guild.voice_states.get(&actor_id) else {
        return CachedFacts::Denied;
    };
    let Some(actor_channel) = actor_state.channel_id else {
        return CachedFacts::Denied;
    };
    let bot_id = cache.current_user().id;
    let Some(bot_channel) = guild
        .voice_states
        .get(&bot_id)
        .and_then(|state| state.channel_id)
    else {
        return CachedFacts::Denied;
    };
    if actor_channel != bot_channel {
        return CachedFacts::Denied;
    }
    let Some(member) = current_cached_member(&guild, actor_id, actor_state.member.as_ref()) else {
        return CachedFacts::Unavailable;
    };
    CachedFacts::Available(VoiceFacts {
        permissions: guild.member_permissions(member),
        roles: member.roles.clone(),
    })
}
