use pepeaudio_core::{ChannelId, UserId};
use serenity::{all, cache::Cache};
use thiserror::Error;

use crate::{GuildControlPolicy, VoiceContext};

/// Reads current authorization facts from Serenity's post-event guild cache.
///
/// Interaction member payloads are intentionally not accepted here because a
/// download or component lifetime can outlive the permissions they captured.
pub(crate) fn current_voice_context(
    cache: &Cache,
    guild_id: all::GuildId,
    actor_id: all::UserId,
    policy: GuildControlPolicy,
) -> Result<VoiceContext, CurrentVoiceFactsError> {
    let guild = cache
        .guild(guild_id)
        .ok_or(CurrentVoiceFactsError::GuildUnavailable)?;
    let actor_state = guild.voice_states.get(&actor_id);
    let member = current_cached_member(
        &guild,
        actor_id,
        actor_state.and_then(|state| state.member.as_ref()),
    )
    .ok_or(CurrentVoiceFactsError::MemberUnavailable)?;
    let actor_voice_channel_id = actor_state
        .and_then(|state| state.channel_id)
        .map(core_channel)
        .transpose()?;
    let bot_voice_channel_id = guild
        .voice_states
        .get(&cache.current_user().id)
        .and_then(|state| state.channel_id)
        .map(core_channel)
        .transpose()?;
    let permissions = guild.member_permissions(member);
    let actor_roles: Vec<u64> = member.roles.iter().map(|role| role.get()).collect();
    Ok(VoiceContext {
        actor_user_id: UserId::new(actor_id.get())
            .map_err(|_| CurrentVoiceFactsError::InvalidSnowflake)?,
        actor_voice_channel_id,
        bot_voice_channel_id,
        has_manage_guild: permissions.manage_guild(),
        has_dj_role: policy.has_dj_role(&actor_roles),
    })
}

/// Selects the freshest cached guild member for every Discord authorization path.
///
/// `GuildMemberUpdate` refreshes `guild.members` but does not rewrite the member
/// clone carried by an older `VoiceState`. The voice-state copy is therefore
/// only a fallback for a partial member cache.
pub(crate) fn current_cached_member<'a>(
    guild: &'a all::Guild,
    actor_id: all::UserId,
    voice_state_member: Option<&'a all::Member>,
) -> Option<&'a all::Member> {
    guild.members.get(&actor_id).or(voice_state_member)
}

fn core_channel(channel_id: all::ChannelId) -> Result<ChannelId, CurrentVoiceFactsError> {
    ChannelId::new(channel_id.get()).map_err(|_| CurrentVoiceFactsError::InvalidSnowflake)
}

#[derive(Clone, Copy, Debug, Error)]
pub(crate) enum CurrentVoiceFactsError {
    #[error("guild voice state is not cached yet")]
    GuildUnavailable,
    #[error("current guild member data is not cached yet")]
    MemberUnavailable,
    #[error("Discord returned an invalid snowflake")]
    InvalidSnowflake,
}

#[cfg(test)]
mod tests {
    use serenity::all::{Guild, GuildId, Member, Permissions, Role, RoleId, UserId};

    use super::current_cached_member;

    #[test]
    fn guild_member_update_revocation_wins_over_stale_voice_state_member() {
        let guild_id = GuildId::new(1);
        let actor_id = UserId::new(2);
        let privileged_role_id = RoleId::new(7);

        let mut guild = Guild::default();
        guild.id = guild_id;
        guild.owner_id = UserId::new(99);

        let mut everyone = Role::default();
        everyone.id = RoleId::new(guild_id.get());
        everyone.guild_id = guild_id;
        guild.roles.insert(everyone.id, everyone);

        let mut privileged_role = Role::default();
        privileged_role.id = privileged_role_id;
        privileged_role.guild_id = guild_id;
        privileged_role.permissions = Permissions::MANAGE_GUILD;
        guild.roles.insert(privileged_role.id, privileged_role);

        let mut stale_voice_member = Member::default();
        stale_voice_member.user.id = actor_id;
        stale_voice_member.guild_id = guild_id;
        stale_voice_member.roles.push(privileged_role_id);

        let mut updated_member = stale_voice_member.clone();
        updated_member.roles.clear();
        guild.members.insert(actor_id, updated_member);

        let selected = current_cached_member(&guild, actor_id, Some(&stale_voice_member))
            .expect("the current guild member is cached");
        assert!(!selected.roles.contains(&privileged_role_id));
        assert!(!guild.member_permissions(selected).manage_guild());
    }
}
