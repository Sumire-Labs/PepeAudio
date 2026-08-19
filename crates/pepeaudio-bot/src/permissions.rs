use pepeaudio_core::{ChannelId, UserId};
use thiserror::Error;

/// Transport-neutral Discord voice and permission facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VoiceContext {
    pub actor_user_id: UserId,
    pub actor_voice_channel_id: Option<ChannelId>,
    pub bot_voice_channel_id: Option<ChannelId>,
    pub has_manage_guild: bool,
    pub has_dj_role: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlPolicy {
    ActorInVoice,
    SameVoiceChannel,
    PrivilegedSameVoiceChannel,
}

/// # Errors
///
/// Returns [`VoicePolicyError`] when the caller lacks the configured privilege.
pub fn authorize_guild_control(
    context: VoiceContext,
    policy: crate::GuildControlPolicy,
) -> Result<ChannelId, VoicePolicyError> {
    let channel = authorize_voice_control(context, ControlPolicy::SameVoiceChannel)?;
    if policy.allows_control(context.has_manage_guild, context.has_dj_role) {
        Ok(channel)
    } else {
        Err(VoicePolicyError::MissingPrivilege)
    }
}

/// # Errors
///
/// Returns [`VoicePolicyError`] when voice membership or privilege checks fail.
pub fn authorize_voice_control(
    context: VoiceContext,
    policy: ControlPolicy,
) -> Result<ChannelId, VoicePolicyError> {
    let actor = context
        .actor_voice_channel_id
        .ok_or(VoicePolicyError::ActorNotInVoice)?;
    match (policy, context.bot_voice_channel_id) {
        (ControlPolicy::ActorInVoice, None) => {}
        (_, Some(bot)) if bot == actor => {}
        _ => return Err(VoicePolicyError::DifferentVoiceChannel),
    }
    if policy == ControlPolicy::PrivilegedSameVoiceChannel
        && !(context.has_manage_guild || context.has_dj_role)
    {
        return Err(VoicePolicyError::MissingPrivilege);
    }
    Ok(actor)
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum VoicePolicyError {
    #[error("join a voice channel before using this control")]
    ActorNotInVoice,
    #[error("you must be in the same voice channel as the bot")]
    DifferentVoiceChannel,
    #[error("this control requires Manage Guild or the configured DJ role")]
    MissingPrivilege,
}

#[cfg(test)]
mod tests {
    use pepeaudio_core::{ChannelId, UserId};

    use super::{
        ControlPolicy, VoiceContext, VoicePolicyError, authorize_guild_control,
        authorize_voice_control,
    };
    use crate::GuildControlPolicy;
    use pepeaudio_storage::ControlPolicy as StoredControlPolicy;

    fn context() -> VoiceContext {
        VoiceContext {
            actor_user_id: UserId::new(1).expect("valid user"),
            actor_voice_channel_id: Some(ChannelId::new(2).expect("valid channel")),
            bot_voice_channel_id: Some(ChannelId::new(2).expect("valid channel")),
            has_manage_guild: false,
            has_dj_role: false,
        }
    }

    #[test]
    fn basic_controls_require_the_same_voice_channel() {
        let mut facts = context();
        facts.bot_voice_channel_id = Some(ChannelId::new(3).expect("valid channel"));
        assert_eq!(
            authorize_voice_control(facts, ControlPolicy::SameVoiceChannel),
            Err(VoicePolicyError::DifferentVoiceChannel)
        );
    }

    #[test]
    fn privileged_controls_accept_manage_guild() {
        let mut facts = context();
        facts.has_manage_guild = true;
        assert!(authorize_voice_control(facts, ControlPolicy::PrivilegedSameVoiceChannel).is_ok());
    }

    #[test]
    fn only_initial_join_policy_allows_an_absent_bot() {
        let mut facts = context();
        facts.bot_voice_channel_id = None;
        assert!(authorize_voice_control(facts, ControlPolicy::ActorInVoice).is_ok());
        assert_eq!(
            authorize_voice_control(facts, ControlPolicy::SameVoiceChannel),
            Err(VoicePolicyError::DifferentVoiceChannel)
        );
    }

    #[test]
    fn manage_guild_policy_is_not_satisfied_by_a_dj_role() {
        let mut facts = context();
        facts.has_dj_role = true;
        let policy = GuildControlPolicy {
            control: StoredControlPolicy::ManageGuild,
            dj_role_id: Some(7),
        };
        assert_eq!(
            authorize_guild_control(facts, policy),
            Err(VoicePolicyError::MissingPrivilege)
        );
    }
}
