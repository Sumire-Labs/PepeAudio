use std::{convert::Infallible, pin::Pin, sync::Arc, time::Duration};

use axum::response::sse::Event;
use futures_util::{Stream, StreamExt as _, stream};
use pepeaudio_core::{GuildId, MAX_PLAYER_SNAPSHOT_JSON_BYTES, PlayerSnapshot, StateRevision};
use serde::{Serialize, ser::Error as _};
use tokio::{
    sync::{broadcast, watch},
    time::{Instant, Interval, Sleep},
};

use crate::sse_admission::SseLease;
use crate::{Access, Authorizer, PlayerEvent, Principal};

pub(crate) type PlayerSseStream = Pin<Box<dyn Stream<Item = Result<Event, Infallible>> + Send>>;

#[derive(Serialize)]
struct SnapshotPayload<'a> {
    revision: StateRevision,
    snapshot: &'a PlayerSnapshot,
}

#[derive(Serialize)]
struct ResyncPayload {
    reason: &'static str,
    last_revision: StateRevision,
    #[serde(skip_serializing_if = "Option::is_none")]
    skipped_events: Option<u64>,
}

struct StreamState {
    guild_id: GuildId,
    initial: Option<Event>,
    receiver: broadcast::Receiver<PlayerEvent>,
    last_revision: StateRevision,
    close_after_event: bool,
}

struct AuthorizationLease {
    inner: PlayerSseStream,
    authorizer: Arc<dyn Authorizer>,
    principal: Principal,
    guild_id: GuildId,
    checks: Interval,
    deadline: Pin<Box<Sleep>>,
}

struct ShutdownLease {
    inner: PlayerSseStream,
    shutdown: watch::Receiver<bool>,
}

struct AdmissionLease {
    inner: PlayerSseStream,
    _lease: SseLease,
}

pub(crate) fn player_stream(
    snapshot: &PlayerSnapshot,
    receiver: broadcast::Receiver<PlayerEvent>,
) -> Result<PlayerSseStream, serde_json::Error> {
    let initial = snapshot_event("snapshot", snapshot)?;
    let state = StreamState {
        guild_id: snapshot.guild_id,
        initial: Some(initial),
        receiver,
        last_revision: snapshot.revision,
        close_after_event: false,
    };

    Ok(Box::pin(stream::unfold(state, |mut state| async move {
        if state.close_after_event {
            return None;
        }
        if let Some(initial) = state.initial.take() {
            return Some((Ok(initial), state));
        }

        loop {
            match state.receiver.recv().await {
                Ok(notification) => {
                    let snapshot = notification.snapshot;
                    if snapshot.guild_id != state.guild_id {
                        let event = resync_event("guild_mismatch", state.last_revision, None);
                        state.close_after_event = true;
                        return Some((Ok(event), state));
                    }
                    if snapshot.revision <= state.last_revision {
                        continue;
                    }
                    if state.last_revision.checked_next() != Some(snapshot.revision) {
                        let event = resync_event("revision_gap", state.last_revision, None);
                        state.close_after_event = true;
                        return Some((Ok(event), state));
                    }
                    let Ok(event) = snapshot_event("player", &snapshot) else {
                        let event =
                            resync_event("serialization_failure", state.last_revision, None);
                        state.close_after_event = true;
                        return Some((Ok(event), state));
                    };
                    state.last_revision = snapshot.revision;
                    return Some((Ok(event), state));
                }
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    let event = resync_event("bounded_lag", state.last_revision, Some(skipped));
                    state.close_after_event = true;
                    return Some((Ok(event), state));
                }
                Err(broadcast::error::RecvError::Closed) => return None,
            }
        }
    })))
}

/// Closes an existing SSE stream when its request-time authorization can no
/// longer be renewed or when the bounded connection lifetime expires.
///
/// The browser reconnects with the opaque cookie and receives a fresh full
/// snapshot. This also re-evaluates new-login session replacement while a
/// long-lived stream is otherwise idle.
pub(crate) fn authorization_guarded_stream(
    inner: PlayerSseStream,
    authorizer: Arc<dyn Authorizer>,
    principal: Principal,
    guild_id: GuildId,
    check_interval: Duration,
    maximum_lifetime: Duration,
) -> PlayerSseStream {
    let now = Instant::now();
    let mut checks = tokio::time::interval_at(now + check_interval, check_interval);
    checks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let state = AuthorizationLease {
        inner,
        authorizer,
        principal,
        guild_id,
        checks,
        deadline: Box::pin(tokio::time::sleep(maximum_lifetime)),
    };

    Box::pin(stream::unfold(state, |mut state| async move {
        loop {
            tokio::select! {
                item = state.inner.next() => return item.map(|item| (item, state)),
                _ = state.checks.tick() => {
                    if state.authorizer.authorize(
                        &state.principal,
                        state.guild_id,
                        Access::SubscribeEvents,
                    ).await.is_err() {
                        return None;
                    }
                }
                () = &mut state.deadline => return None,
            }
        }
    }))
}

/// Finishes a streaming response when the API process starts shutting down.
///
/// This guard is intentionally independent from authorization renewal: either
/// boundary may close the stream, and reconnect behavior remains a client
/// concern only while the process is accepting requests.
pub(crate) fn shutdown_guarded_stream(
    inner: PlayerSseStream,
    shutdown: watch::Receiver<bool>,
) -> PlayerSseStream {
    let state = ShutdownLease { inner, shutdown };
    Box::pin(stream::unfold(state, |mut state| async move {
        let inner = &mut state.inner;
        let shutdown = &mut state.shutdown;
        tokio::select! {
            biased;
            () = shutdown_requested(shutdown) => None,
            item = inner.next() => item.map(|item| (item, state)),
        }
    }))
}

/// Retains process-local admission until the response stream is dropped or
/// reaches a terminal boundary.
pub(crate) fn admission_guarded_stream(inner: PlayerSseStream, lease: SseLease) -> PlayerSseStream {
    let state = AdmissionLease {
        inner,
        _lease: lease,
    };
    Box::pin(stream::unfold(state, |mut state| async move {
        state.inner.next().await.map(|item| (item, state))
    }))
}

async fn shutdown_requested(shutdown: &mut watch::Receiver<bool>) {
    loop {
        if *shutdown.borrow_and_update() {
            return;
        }
        if shutdown.changed().await.is_err() {
            return;
        }
    }
}

fn snapshot_event(
    kind: &'static str,
    snapshot: &PlayerSnapshot,
) -> Result<Event, serde_json::Error> {
    snapshot
        .validate_public_shape()
        .map_err(|_| serde_json::Error::custom("player snapshot shape is invalid"))?;
    let data = serde_json::to_string(&SnapshotPayload {
        revision: snapshot.revision,
        snapshot,
    })?;
    if data.len() > MAX_PLAYER_SNAPSHOT_JSON_BYTES {
        return Err(serde_json::Error::custom(
            "player snapshot exceeds the SSE frame limit",
        ));
    }
    Ok(Event::default()
        .event(kind)
        .id(snapshot.revision.get().to_string())
        .data(data))
}

fn resync_event(
    reason: &'static str,
    last_revision: StateRevision,
    skipped_events: Option<u64>,
) -> Event {
    let payload = ResyncPayload {
        reason,
        last_revision,
        skipped_events,
    };
    let data = serde_json::to_string(&payload).unwrap_or_else(|_| {
        "{\"reason\":\"serialization_failure\",\"last_revision\":0}".to_owned()
    });
    Event::default().event("resync").data(data)
}

#[cfg(test)]
mod tests;
