use std::time::Duration;

use axum::{
    extract::{Path, State},
    http::HeaderMap,
    response::{
        IntoResponse, Response,
        sse::{KeepAlive, Sse},
    },
};

use crate::{
    Access, ApiError, AppState, Principal,
    initial_snapshot::initial_snapshot,
    sse::{admission_guarded_stream, authorization_guarded_stream, player_stream},
    validation::{guild_id, last_event_id},
};

const AUTHORIZATION_CHECK_INTERVAL: Duration = Duration::from_mins(1);
const MAXIMUM_STREAM_LIFETIME: Duration = Duration::from_mins(5);

pub(crate) async fn events(
    State(state): State<AppState>,
    Path(guild): Path<String>,
    headers: HeaderMap,
    principal: Principal,
) -> Result<Response, ApiError> {
    let guild_id = guild_id(&guild)?;
    let _client_revision = last_event_id(&headers)?;
    state
        .authorizer
        .authorize(&principal, guild_id, Access::SubscribeEvents)
        .await
        .map_err(ApiError::from)?;
    let admission = state
        .sse_admission
        .acquire(principal.user_id())
        .ok_or(ApiError::SseCapacityExceeded)?;

    // Subscribe first, then fetch the full snapshot. Events published during
    // the fetch are either duplicate/stale and skipped or reveal a revision gap
    // that causes an explicit resync boundary.
    let receiver = state.events.subscribe(guild_id).map_err(ApiError::from)?;
    let snapshot = state
        .snapshots
        .snapshot(guild_id)
        .await
        .map_err(ApiError::from)?
        .unwrap_or_else(|| initial_snapshot(guild_id, state.clock.now()));
    if snapshot.guild_id != guild_id {
        return Err(ApiError::Internal);
    }
    let stream = player_stream(&snapshot, receiver).map_err(|_| ApiError::Internal)?;
    let stream = authorization_guarded_stream(
        stream,
        state.authorizer.clone(),
        principal,
        guild_id,
        AUTHORIZATION_CHECK_INTERVAL,
        MAXIMUM_STREAM_LIFETIME,
    );
    let stream = crate::sse::shutdown_guarded_stream(stream, state.shutdown.subscribe());
    let stream = admission_guarded_stream(stream, admission);
    Ok(Sse::new(stream)
        .keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(15))
                .text("keep-alive"),
        )
        .into_response())
}
