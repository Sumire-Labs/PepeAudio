use crate::{
    CommandError, Context, build_now_panel,
    commands::{guild_id, response::edit_deferred},
};

/// 再生状況と操作パネルを表示します。
#[poise::command(slash_command, guild_only)]
pub(crate) async fn now(ctx: Context<'_>) -> Result<(), CommandError> {
    ctx.defer().await?;
    let guild_id = guild_id(ctx)?;
    let player = ctx
        .data()
        .players
        .get(guild_id)
        .await
        .ok_or("this server has no active player")?;
    let snapshot = player.snapshot().await?;
    let panel = build_now_panel(
        &snapshot,
        &ctx.data().component_ids,
        &ctx.data().hrir_options,
    )?;
    let message_id = edit_deferred(ctx, &panel).await?;
    ctx.data()
        .now_panels
        .track(guild_id, ctx.channel_id(), message_id, player, &snapshot);
    Ok(())
}
