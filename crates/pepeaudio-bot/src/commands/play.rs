use std::future::Future;

use poise::serenity_prelude as serenity;

use crate::{
    AttachmentSource, CommandError, Context, ControlPolicy, authorize_voice_control,
    build_status_panel,
    commands::{
        guild_id,
        response::{applied_response_error, edit_deferred_after_apply},
        voice_context,
    },
    display_text::escape_discord_markdown,
};

const MAX_COMMIT_ATTEMPTS: usize = 4;

#[derive(Debug, Eq, PartialEq, thiserror::Error)]
pub(crate) enum PlayInputError {
    #[error("a URL or an attachment is required")]
    Missing,
    #[error("a URL and an attachment cannot be supplied together")]
    Conflicting,
}

#[derive(Debug, Eq, PartialEq)]
enum PlaySource {
    Input(String),
    Attachment(AttachmentSource),
}

/// 曲名、URL、または添付ファイルを再生キューに追加します。
#[poise::command(slash_command, guild_only)]
pub(crate) async fn play(
    ctx: Context<'_>,
    #[description = "曲名または対応URL（/playは入力せず検索語かURLだけ入力）"]
    #[max_length = 4096]
    query: Option<String>,
    #[description = "再生する音声ファイル"] file: Option<serenity::Attachment>,
) -> Result<(), CommandError> {
    ctx.defer().await?;
    let source = select_play_source(query, file.map(attachment_source))?;
    let guild_id = guild_id(ctx)?;
    let (voice, policy) = voice_context(ctx).await?;
    authorize_play(voice, policy)?;
    let player = authorized_player(&ctx.data().players, guild_id, voice_context(ctx).await).await?;
    let before = player.snapshot().await?;
    let maximum_items = available_batch_items(ctx.data().media.as_ref(), &before)?;
    let batch = match source {
        PlaySource::Input(input) => {
            ctx.data()
                .media
                .resolve_input(guild_id, voice.actor_user_id, &input, maximum_items)
                .await?
        }
        PlaySource::Attachment(attachment) => {
            ctx.data()
                .media
                .resolve_attachment(guild_id, voice.actor_user_id, attachment)
                .await?
        }
    };
    finish_enqueue(ctx, player, batch).await
}

fn select_play_source(
    input: Option<String>,
    attachment: Option<AttachmentSource>,
) -> Result<PlaySource, PlayInputError> {
    match (input, attachment) {
        (Some(input), None) => Ok(PlaySource::Input(input)),
        (None, Some(attachment)) => Ok(PlaySource::Attachment(attachment)),
        (None, None) => Err(PlayInputError::Missing),
        (Some(_), Some(_)) => Err(PlayInputError::Conflicting),
    }
}

fn attachment_source(file: serenity::Attachment) -> AttachmentSource {
    AttachmentSource {
        filename: file.filename,
        url: file.url,
        content_type: file.content_type,
        size_bytes: u64::from(file.size),
    }
}

async fn finish_enqueue(
    ctx: Context<'_>,
    player: pepeaudio_player::PlayerHandle,
    batch: crate::ResolvedMediaBatch,
) -> Result<(), CommandError> {
    let pending_batch = batch;
    let first_track_id = pending_batch
        .tracks
        .first()
        .ok_or("the media resolver returned an empty batch")?
        .track_id;
    // Media resolution may take long enough for a VC move or role revocation.
    // Authorize before actor creation, then again after potentially slow actor
    // initialization and immediately before the first player mutation.
    let enqueue_result = retry_revision_conflicts(|| async {
        let refreshed = voice_context(ctx).await;
        commit_resolved_tracks(&player, &pending_batch.tracks, refreshed).await
    })
    .await;
    let snapshot = match enqueue_result {
        Ok(snapshot) => snapshot,
        Err(error) => {
            if ctx
                .data()
                .media
                .discard_uncommitted(pending_batch)
                .await
                .is_err()
            {
                tracing::warn!(
                    guild_id = ctx.guild_id().map_or(0, serenity::GuildId::get),
                    "uncommitted media cleanup remains pending for the janitor"
                );
            }
            return Err(error);
        }
    };

    // The player owns the managed file after enqueue succeeds. A Discord REST
    // acknowledgement failure must never delete media that is already queued.
    let message = if snapshot
        .current_track
        .as_ref()
        .is_some_and(|current| current.track_id == first_track_id)
    {
        if pending_batch.tracks.len() == 1 {
            "## 再生を開始しました\nボイスチャンネルで再生しています。".to_owned()
        } else {
            format!(
                "## 再生を開始しました\n先頭の曲を再生し、残り{}曲をキューへ追加しました。{}",
                pending_batch.tracks.len().saturating_sub(1),
                import_notice(&pending_batch)
            )
        }
    } else {
        format!(
            "## キューに追加しました\n{}曲を追加しました。再生待ち: `{}曲`{}",
            pending_batch.tracks.len(),
            snapshot.queued_tracks,
            import_notice(&pending_batch)
        )
    };
    let panel = build_status_panel(message).map_err(applied_response_error)?;
    edit_deferred_after_apply(ctx, &panel).await
}

async fn commit_resolved_tracks(
    player: &pepeaudio_player::PlayerHandle,
    tracks: &[pepeaudio_player::QueueTrack],
    current_voice: Result<(crate::VoiceContext, crate::GuildControlPolicy), CommandError>,
) -> Result<pepeaudio_core::PlayerSnapshot, CommandError> {
    let (voice, policy) = current_voice?;
    let channel = authorize_play(voice, policy)?;
    let before = player.snapshot().await?;
    match (before.voice_channel_id, voice.bot_voice_channel_id) {
        (None, None) => {
            player.connect(channel, before.revision).await?;
        }
        (Some(actor_channel), Some(discord_channel))
            if actor_channel == channel && discord_channel == channel => {}
        (Some(_), Some(_)) => {
            return Err("the player is active in another voice channel".into());
        }
        (Some(_), None) | (None, Some(_)) => {
            return Err("the Discord voice state is still being reconciled; retry shortly".into());
        }
    }
    let snapshot = player.snapshot().await?;
    Ok(player
        .enqueue_batch(tracks.to_vec(), snapshot.revision)
        .await?)
}

pub(crate) fn available_batch_items(
    media: &dyn crate::MediaResolver,
    snapshot: &pepeaudio_core::PlayerSnapshot,
) -> Result<usize, CommandError> {
    let queued = usize::try_from(snapshot.queued_tracks).unwrap_or(usize::MAX);
    let available = if snapshot.current_track.is_none() {
        media
            .queue_capacity()
            .saturating_add(1)
            .saturating_sub(queued)
    } else {
        media.queue_capacity().saturating_sub(queued)
    };
    let maximum = available.min(media.maximum_playlist_items());
    if maximum == 0 {
        Err("the playback queue is full".into())
    } else {
        Ok(maximum)
    }
}

fn import_notice(batch: &crate::ResolvedMediaBatch) -> String {
    let mut notices = Vec::new();
    if let Some(title) = batch.source_title.as_deref() {
        let title = escape_discord_markdown(title);
        notices.push(format!("コレクション: {title}"));
    }
    if batch.skipped_items > 0 {
        notices.push(format!("未対応の{}件を除外", batch.skipped_items));
    }
    if batch.truncated {
        let processed = batch.tracks.len().saturating_add(batch.skipped_items);
        notices.push(batch.source_item_count.map_or_else(
            || format!("先頭{processed}件までを処理し、残りを省略"),
            |count| format!("全{count}件のうち先頭{processed}件を処理"),
        ));
    }
    if notices.is_empty() {
        String::new()
    } else {
        format!("\n{}。", notices.join(" / "))
    }
}

async fn retry_revision_conflicts<T, F, Fut>(mut attempt: F) -> Result<T, CommandError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, CommandError>>,
{
    for attempt_index in 0..MAX_COMMIT_ATTEMPTS {
        match attempt().await {
            Err(error)
                if is_revision_conflict(error.as_ref())
                    && attempt_index + 1 < MAX_COMMIT_ATTEMPTS =>
            {
                tokio::task::yield_now().await;
            }
            result => return result,
        }
    }
    unreachable!("the bounded retry loop always returns on its final attempt")
}

fn is_revision_conflict(error: &(dyn std::error::Error + Send + Sync + 'static)) -> bool {
    error
        .downcast_ref::<pepeaudio_player::PlayerError>()
        .is_some_and(|error| {
            matches!(
                error,
                pepeaudio_player::PlayerError::RevisionConflict { .. }
                    | pepeaudio_player::PlayerError::InvalidCommand(
                        pepeaudio_core::CommandValidationError::RevisionConflict { .. }
                    )
            )
        })
}

async fn authorized_player(
    players: &crate::PlayerRegistry,
    guild_id: pepeaudio_core::GuildId,
    current_voice: Result<(crate::VoiceContext, crate::GuildControlPolicy), CommandError>,
) -> Result<pepeaudio_player::PlayerHandle, CommandError> {
    let (voice, policy) = current_voice?;
    authorize_play(voice, policy)?;
    Ok(players.get_or_create(guild_id).await?)
}

fn authorize_play(
    voice: crate::VoiceContext,
    policy: crate::GuildControlPolicy,
) -> Result<pepeaudio_core::ChannelId, crate::VoicePolicyError> {
    if !policy.allows_control(voice.has_manage_guild, voice.has_dj_role) {
        return Err(crate::VoicePolicyError::MissingPrivilege);
    }
    authorize_voice_control(voice, ControlPolicy::ActorInVoice)
}

#[cfg(test)]
#[path = "play_tests.rs"]
mod tests;
