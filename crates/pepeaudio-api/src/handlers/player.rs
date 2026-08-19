use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use pepeaudio_core::{
    CommandEnvelope, CommandResultStatus, MAX_PLAYER_SNAPSHOT_JSON_BYTES, PlayerCommand,
    StateRevision,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    Access, ApiError, AppState, Principal,
    validation::{command_deadline, guild_id, validate_mutation_headers},
};

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct MutationRequest {
    expected_revision: StateRevision,
    command: PlayerCommand,
}

pub(crate) async fn snapshot(
    State(state): State<AppState>,
    Path(guild): Path<String>,
    principal: Principal,
) -> Result<Response, ApiError> {
    let guild_id = guild_id(&guild)?;
    state
        .authorizer
        .authorize(&principal, guild_id, Access::ReadPlayer)
        .await
        .map_err(ApiError::from)?;
    let snapshot = state
        .snapshots
        .snapshot(guild_id)
        .await
        .map_err(ApiError::from)?
        .ok_or(ApiError::NotFound)?;
    let encoded_within_limit = matches!(
        serde_json::to_vec(&snapshot),
        Ok(encoded) if encoded.len() <= MAX_PLAYER_SNAPSHOT_JSON_BYTES
    );
    if snapshot.guild_id != guild_id
        || snapshot.validate_public_shape().is_err()
        || !encoded_within_limit
    {
        return Err(ApiError::Internal);
    }
    Ok(Json(snapshot).into_response())
}

pub(crate) async fn mutate(
    State(state): State<AppState>,
    Path(guild): Path<String>,
    headers: HeaderMap,
    principal: Principal,
    payload: Result<Json<MutationRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    let guild_id = guild_id(&guild)?;
    let idempotency_key = validate_mutation_headers(&headers, &state.config, &principal)?;
    state
        .authorizer
        .authorize(&principal, guild_id, Access::ControlPlayer)
        .await
        .map_err(ApiError::from)?;
    let Json(payload) = payload.map_err(|_| ApiError::invalid_request())?;
    let now = state.clock.now();
    let deadline = command_deadline(now, &state.config)?;
    let mut envelope = CommandEnvelope::new(
        guild_id,
        Some(principal.user_id()),
        payload.expected_revision,
        deadline,
        payload.command,
    );
    envelope.idempotency_key = idempotency_key;

    let receipt = state
        .commands
        .route(envelope, now)
        .await
        .map_err(ApiError::from)?;
    Ok((StatusCode::ACCEPTED, Json(receipt)).into_response())
}

pub(crate) async fn command_result(
    State(state): State<AppState>,
    Path((guild, command)): Path<(String, String)>,
    principal: Principal,
) -> Result<Response, ApiError> {
    let guild_id = guild_id(&guild)?;
    let command_id = command
        .parse::<Uuid>()
        .ok()
        .filter(|id| !id.is_nil())
        .ok_or_else(ApiError::invalid_request)?;
    state
        .authorizer
        .authorize(&principal, guild_id, Access::ReadPlayer)
        .await
        .map_err(ApiError::from)?;
    let result = state
        .command_results
        .command_result(guild_id, command_id)
        .await
        .map_err(ApiError::from)?
        .ok_or(ApiError::CommandResultNotFound)?;
    if result.guild_id != guild_id || result.command_id != command_id {
        return Err(ApiError::Internal);
    }
    let status = if matches!(result.status, CommandResultStatus::Pending) {
        StatusCode::ACCEPTED
    } else {
        StatusCode::OK
    };
    Ok((status, Json(result)).into_response())
}
