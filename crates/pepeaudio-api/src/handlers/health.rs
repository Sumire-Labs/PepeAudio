use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

use crate::{ApiError, AppState};

#[derive(Clone, Copy, Debug, Serialize)]
pub(crate) struct HealthBody {
    status: &'static str,
}

pub(crate) async fn live() -> Json<HealthBody> {
    Json(HealthBody { status: "live" })
}

pub(crate) async fn ready(State(state): State<AppState>) -> Result<Response, ApiError> {
    state.readiness.ready().await.map_err(ApiError::from)?;
    Ok((StatusCode::OK, Json(HealthBody { status: "ready" })).into_response())
}
