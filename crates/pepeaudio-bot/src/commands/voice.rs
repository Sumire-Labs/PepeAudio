use pepeaudio_core::GuildId;

use crate::{CommandError, Context, VoiceContext, voice_facts::current_voice_context};

pub(crate) fn guild_id(ctx: Context<'_>) -> Result<GuildId, CommandError> {
    let id = ctx
        .guild_id()
        .ok_or_else(|| "this command is only available in a server".to_owned())?;
    Ok(GuildId::new(id.get())?)
}

pub(crate) async fn voice_context(
    ctx: Context<'_>,
) -> Result<(VoiceContext, crate::GuildControlPolicy), CommandError> {
    let guild_id = ctx
        .guild_id()
        .ok_or_else(|| "this command is only available in a server".to_owned())?;
    let guild_id = GuildId::new(guild_id.get())?;
    let policy = ctx.data().guild_policy.policy(guild_id).await?;
    let voice = current_voice_context(
        &ctx.serenity_context().cache,
        poise::serenity_prelude::GuildId::new(guild_id.get()),
        ctx.author().id,
        policy,
    )?;
    Ok((voice, policy))
}
