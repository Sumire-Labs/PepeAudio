use crate::{
    CommandError, Context, ControlPolicy, authorize_voice_control, build_status_panel,
    commands::{
        guild_id,
        response::{applied_response_error, edit_deferred_after_apply},
        voice_context,
    },
};

/// 再生を終了し、ボイスチャンネルから退出します。
#[poise::command(slash_command, guild_only)]
pub(crate) async fn leave(ctx: Context<'_>) -> Result<(), CommandError> {
    ctx.defer().await?;
    let guild_id = guild_id(ctx)?;
    let (voice, _policy) = voice_context(ctx).await?;
    authorize_voice_control(voice, ControlPolicy::PrivilegedSameVoiceChannel)?;
    let player = ctx
        .data()
        .players
        .get(guild_id)
        .await
        .ok_or("this server has no active player")?;
    let snapshot = player.snapshot().await?;
    super::stop::ensure_active_channel(snapshot.voice_channel_id, voice.actor_voice_channel_id)?;

    if !ctx.data().players.remove_and_shutdown(guild_id).await? {
        return Err("this server has no active player".into());
    }
    let panel = build_status_panel(
        "## ボイスチャンネルから退出しました\n再生中の曲とキューを終了しました。",
    )
    .map_err(applied_response_error)?;
    edit_deferred_after_apply(ctx, &panel).await
}
