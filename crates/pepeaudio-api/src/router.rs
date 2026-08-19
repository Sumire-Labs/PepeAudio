use axum::{
    Router,
    extract::DefaultBodyLimit,
    http::{HeaderValue, Request, header::CACHE_CONTROL},
    middleware::{self, Next},
    response::Response,
    routing::{get, post},
};

use crate::{ApiError, AppState, handlers};

/// Builds the HTTP application without permissive CORS middleware.
///
/// State-changing requests enforce one exact configured `Origin`, same-origin
/// Fetch Metadata, and a session-bound CSRF token. Deployments should expose
/// this router same-origin behind Caddy.
pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health/live", get(handlers::health::live))
        .route("/health/ready", get(handlers::health::ready))
        .route(
            "/api/v1/guilds/{guild_id}/player",
            get(handlers::player::snapshot),
        )
        .route(
            "/api/v1/guilds/{guild_id}/player/commands",
            post(handlers::player::mutate),
        )
        .route(
            "/api/v1/guilds/{guild_id}/player/commands/{command_id}",
            get(handlers::player::command_result),
        )
        .route(
            "/api/v1/guilds/{guild_id}/hrir-presets",
            get(handlers::hrir_presets::list),
        )
        .route(
            "/api/v1/guilds/{guild_id}/events",
            get(handlers::events::events),
        )
        .fallback(|| async { ApiError::NotFound })
        .layer(middleware::from_fn(private_no_store))
        .layer(DefaultBodyLimit::max(64 * 1024))
        .with_state(state)
}

async fn private_no_store(request: Request<axum::body::Body>, next: Next) -> Response {
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("private, no-store"));
    response
}
