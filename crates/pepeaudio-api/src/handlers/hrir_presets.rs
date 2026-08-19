use axum::{
    Json,
    extract::{Path, State},
    response::{IntoResponse, Response},
};

use crate::{Access, ApiError, AppState, Principal, validation::guild_id};

pub(crate) async fn list(
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
    let catalog = state
        .hrir_presets
        .hrir_presets(guild_id)
        .await
        .map_err(ApiError::from)?;
    if catalog.guild_id != guild_id {
        return Err(ApiError::Internal);
    }
    Ok(Json(catalog).into_response())
}
