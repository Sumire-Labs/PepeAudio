use std::time::{SystemTime, UNIX_EPOCH};

use pepeaudio_core::{CommandEnvelope, PlayerCommand, UnixTimeMillis};

use crate::{
    CommandError, Context, ControlPolicy, authorize_voice_control, build_status_panel,
    commands::{
        guild_id,
        response::{applied_response_error, edit_deferred_after_apply},
        voice_context,
    },
};

/// 再生を停止してキューを空にします。
#[poise::command(slash_command, guild_only)]
pub(crate) async fn stop(ctx: Context<'_>) -> Result<(), CommandError> {
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
    ensure_active_channel(snapshot.voice_channel_id, voice.actor_voice_channel_id)?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
    let now = u64::try_from(now).unwrap_or(u64::MAX - 30_000);
    let envelope = CommandEnvelope::new(
        guild_id,
        Some(voice.actor_user_id),
        snapshot.revision,
        UnixTimeMillis::new(now.saturating_add(30_000)),
        PlayerCommand::Stop,
    );
    player.apply(envelope).await?;
    let panel = build_status_panel("## 再生を停止しました\nキューも空にしました。")
        .map_err(applied_response_error)?;
    edit_deferred_after_apply(ctx, &panel).await
}

pub(super) fn ensure_active_channel(
    player_channel: Option<pepeaudio_core::ChannelId>,
    actor_channel: Option<pepeaudio_core::ChannelId>,
) -> Result<(), CommandError> {
    if player_channel == actor_channel && player_channel.is_some() {
        Ok(())
    } else {
        Err("you must be in the player's active voice channel".into())
    }
}

#[cfg(test)]
mod tests {
    use pepeaudio_core::ChannelId;

    use super::ensure_active_channel;

    #[test]
    fn active_channel_check_fails_closed_on_mismatch_or_missing_state() {
        let active = ChannelId::new(1).expect("channel");
        let other = ChannelId::new(2).expect("channel");
        assert!(ensure_active_channel(Some(active), Some(active)).is_ok());
        assert!(ensure_active_channel(Some(active), Some(other)).is_err());
        assert!(ensure_active_channel(None, None).is_err());
    }
}
