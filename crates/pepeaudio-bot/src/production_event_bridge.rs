use std::time::Duration;

use pepeaudio_core::GuildId;
use pepeaudio_pipeline::{PlaybackEndReason as PipelineEndReason, PlaybackEvent};
use pepeaudio_player::{PlaybackEndReason, PlayerError, PlayerEvent, PlayerHandle};
use tokio::{sync::broadcast, time::timeout};

const CLEANUP_ATTEMPT_TIMEOUT: Duration = Duration::from_secs(15);
const INITIAL_CLEANUP_BACKOFF: Duration = Duration::from_secs(1);
const MAX_CLEANUP_BACKOFF: Duration = Duration::from_secs(30);

/// Forwards finite pipeline outcomes into the authoritative guild actor.
pub(crate) fn spawn(
    guild_id: GuildId,
    playback_events: broadcast::Receiver<PlaybackEvent>,
    handle: PlayerHandle,
) {
    tokio::spawn(run(guild_id, playback_events, handle));
}

pub(super) async fn run(
    guild_id: GuildId,
    mut playback_events: broadcast::Receiver<PlaybackEvent>,
    handle: PlayerHandle,
) {
    let mut lifecycle = handle.subscribe();
    let recovery_reason = loop {
        tokio::select! {
            biased;
            event = lifecycle.recv() => {
                if lifecycle_stopped(&event) {
                    return;
                }
            }
            event = playback_events.recv() => match event {
                Ok(PlaybackEvent::TrackEnded { identity, reason, .. }) => {
                    if let Err(error) = handle.playback_ended(identity, map_reason(reason)).await {
                        tracing::error!(
                            guild_id = guild_id.get(),
                            error = %error,
                            "automatic queue advance failed; entering player cleanup recovery"
                        );
                        break "queue advance failed";
                    }
                }
                Ok(PlaybackEvent::WorkerFailed { identity, .. }) => {
                    tracing::warn!(
                        guild_id = guild_id.get(),
                        "audio worker reported a failure; advancing the queue"
                    );
                    if let Err(error) = handle
                        .playback_ended(identity, PlaybackEndReason::WorkerFailed)
                        .await
                    {
                        tracing::error!(
                            guild_id = guild_id.get(),
                            error = %error,
                            "failed-worker queue advance failed; entering player cleanup recovery"
                        );
                        break "failed-worker queue advance failed";
                    }
                }
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    tracing::error!(
                        guild_id = guild_id.get(),
                        skipped,
                        "audio lifecycle events lagged; entering player cleanup recovery"
                    );
                    break "pipeline event stream lagged";
                }
                Err(broadcast::error::RecvError::Closed) => {
                    tracing::error!(
                        guild_id = guild_id.get(),
                        "audio lifecycle event stream closed; entering player cleanup recovery"
                    );
                    break "pipeline event stream closed";
                }
            }
        }
    };

    recover_player(guild_id, recovery_reason, &handle, &mut lifecycle).await;
}

async fn recover_player(
    guild_id: GuildId,
    reason: &'static str,
    handle: &PlayerHandle,
    lifecycle: &mut broadcast::Receiver<PlayerEvent>,
) {
    let mut attempt = 1_u64;
    let mut backoff = INITIAL_CLEANUP_BACKOFF;
    loop {
        let shutdown = timeout(CLEANUP_ATTEMPT_TIMEOUT, handle.shutdown());
        tokio::pin!(shutdown);
        let outcome = loop {
            tokio::select! {
                biased;
                event = lifecycle.recv() => {
                    if lifecycle_stopped(&event) {
                        return;
                    }
                }
                result = &mut shutdown => break result,
            }
        };

        match outcome {
            Ok(Ok(report)) if report.disconnect_error.is_none() => return,
            Ok(Ok(report)) => tracing::warn!(
                guild_id = guild_id.get(),
                attempt,
                reason,
                error = report
                    .disconnect_error
                    .as_deref()
                    .unwrap_or("unknown cleanup error"),
                retry_after_ms = backoff.as_millis(),
                "player cleanup was not confirmed; retrying"
            ),
            Ok(Err(PlayerError::ActorStopped)) => return,
            Ok(Err(error)) => tracing::warn!(
                guild_id = guild_id.get(),
                attempt,
                reason,
                error = %error,
                retry_after_ms = backoff.as_millis(),
                "player cleanup request failed; retrying"
            ),
            Err(_) => tracing::warn!(
                guild_id = guild_id.get(),
                attempt,
                reason,
                timeout_ms = CLEANUP_ATTEMPT_TIMEOUT.as_millis(),
                retry_after_ms = backoff.as_millis(),
                "player cleanup request timed out; retrying"
            ),
        }

        let delay = tokio::time::sleep(backoff);
        tokio::pin!(delay);
        loop {
            tokio::select! {
                biased;
                event = lifecycle.recv() => {
                    if lifecycle_stopped(&event) {
                        return;
                    }
                }
                () = &mut delay => break,
            }
        }
        attempt = attempt.saturating_add(1);
        backoff = backoff.saturating_mul(2).min(MAX_CLEANUP_BACKOFF);
    }
}

fn lifecycle_stopped(event: &Result<PlayerEvent, broadcast::error::RecvError>) -> bool {
    matches!(
        event,
        Ok(PlayerEvent::Shutdown(_)) | Err(broadcast::error::RecvError::Closed)
    )
}

const fn map_reason(reason: PipelineEndReason) -> PlaybackEndReason {
    match reason {
        PipelineEndReason::Natural => PlaybackEndReason::Natural,
        PipelineEndReason::WorkerFailed => PlaybackEndReason::WorkerFailed,
        PipelineEndReason::SongbirdEnded => PlaybackEndReason::SongbirdEnded,
        PipelineEndReason::SongbirdError => PlaybackEndReason::SongbirdError,
    }
}
