use axum::{
    Json, Router,
    extract::{Query, State},
    http::{
        HeaderMap, HeaderValue, Request, StatusCode,
        header::{CACHE_CONTROL, LOCATION, REFERRER_POLICY, SET_COOKIE},
    },
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};

use crate::{
    AuthService, SessionView,
    cookie::{
        clear_session_cookie, clear_state_cookie, oauth_state_cookie, session_cookie,
        session_set_cookie, state_set_cookie, validate_csrf,
    },
    service::AuthServiceError,
};

pub fn build_auth_router(service: AuthService) -> Router {
    Router::new()
        .route("/auth/login", get(login))
        .route("/auth/callback", get(callback))
        .route("/auth/logout", post(logout))
        .route("/auth/session", get(session))
        .route("/auth/guilds", get(guilds))
        .layer(middleware::from_fn(security_headers))
        .with_state(service)
}

async fn login(State(service): State<AuthService>) -> Result<Response, AuthHttpError> {
    let started = service.begin_login().await?;
    let location = HeaderValue::from_str(started.authorization_url.as_str())
        .map_err(|_| AuthHttpError::Unavailable)?;
    let mut response = StatusCode::SEE_OTHER.into_response();
    response.headers_mut().insert(LOCATION, location);
    response.headers_mut().append(
        SET_COOKIE,
        state_set_cookie(&started.state, service.config.session.oauth_state_ttl),
    );
    Ok(response)
}

async fn callback(
    State(service): State<AuthService>,
    headers: HeaderMap,
    Query(query): Query<CallbackQuery>,
) -> Response {
    let cookie_state = oauth_state_cookie(&headers).ok().flatten();
    if query.error.is_some() {
        if let (Some(state), Some(cookie)) = (query.state.as_deref(), cookie_state) {
            service.consume_denied_callback(state, cookie).await;
        }
        return callback_error(AuthHttpError::InvalidCallback);
    }
    let Some(code) = query.code.as_deref() else {
        return callback_error(AuthHttpError::InvalidCallback);
    };
    let Some(returned_state) = query.state.as_deref() else {
        return callback_error(AuthHttpError::InvalidCallback);
    };
    let Some(cookie_state) = cookie_state else {
        return callback_error(AuthHttpError::InvalidCallback);
    };
    match service
        .complete_callback(code, returned_state, cookie_state)
        .await
    {
        Ok(session_token) => {
            let mut response = StatusCode::SEE_OTHER.into_response();
            let Ok(location) = HeaderValue::from_str(&service.config.success_redirect) else {
                return callback_error(AuthHttpError::Unavailable);
            };
            response.headers_mut().insert(LOCATION, location);
            response.headers_mut().append(
                SET_COOKIE,
                session_set_cookie(&session_token, service.config.session.absolute_ttl),
            );
            response
                .headers_mut()
                .append(SET_COOKIE, clear_state_cookie());
            response
        }
        Err(error) => callback_error(error.into()),
    }
}

async fn logout(
    State(service): State<AuthService>,
    headers: HeaderMap,
) -> Result<Response, AuthHttpError> {
    let token = required_session_cookie(&headers)?;
    let session = service.load_session(token).await?;
    if !validate_csrf(&headers, &session.csrf_token) {
        return Err(AuthHttpError::Forbidden);
    }
    service.logout(token).await?;
    let mut response = StatusCode::NO_CONTENT.into_response();
    response
        .headers_mut()
        .append(SET_COOKIE, clear_session_cookie());
    Ok(response)
}

async fn session(
    State(service): State<AuthService>,
    headers: HeaderMap,
) -> Result<Json<SessionView>, AuthHttpError> {
    let token = required_session_cookie(&headers)?;
    Ok(Json(service.session_view(token).await?))
}

async fn guilds(
    State(service): State<AuthService>,
    headers: HeaderMap,
) -> Result<Json<GuildList>, AuthHttpError> {
    let token = required_session_cookie(&headers)?;
    Ok(Json(GuildList {
        guilds: service.guild_views(token).await?,
    }))
}

fn required_session_cookie(headers: &HeaderMap) -> Result<&str, AuthHttpError> {
    session_cookie(headers)
        .map_err(|_| AuthHttpError::Unauthenticated)?
        .ok_or(AuthHttpError::Unauthenticated)
}

fn callback_error(error: AuthHttpError) -> Response {
    let mut response = error.into_response();
    response
        .headers_mut()
        .append(SET_COOKIE, clear_state_cookie());
    response
}

async fn security_headers(request: Request<axum::body::Body>, next: Next) -> Response {
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("private, no-store"));
    response
        .headers_mut()
        .insert(REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    response
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
    #[allow(dead_code)]
    error_description: Option<String>,
}

#[derive(Serialize)]
struct GuildList {
    guilds: Vec<crate::GuildView>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AuthHttpError {
    Unauthenticated,
    Forbidden,
    InvalidCallback,
    Unavailable,
    CapacityExceeded,
}

impl From<AuthServiceError> for AuthHttpError {
    fn from(value: AuthServiceError) -> Self {
        match value {
            AuthServiceError::Unauthenticated => Self::Unauthenticated,
            AuthServiceError::InvalidCallback => Self::InvalidCallback,
            AuthServiceError::Unavailable => Self::Unavailable,
            AuthServiceError::CapacityExceeded => Self::CapacityExceeded,
        }
    }
}

impl IntoResponse for AuthHttpError {
    fn into_response(self) -> Response {
        let (status, code) = match self {
            Self::Unauthenticated => (StatusCode::UNAUTHORIZED, "authentication_required"),
            Self::Forbidden => (StatusCode::FORBIDDEN, "csrf_rejected"),
            Self::InvalidCallback => (StatusCode::BAD_REQUEST, "invalid_oauth_callback"),
            Self::Unavailable => (
                StatusCode::SERVICE_UNAVAILABLE,
                "authentication_unavailable",
            ),
            Self::CapacityExceeded => (
                StatusCode::TOO_MANY_REQUESTS,
                "authentication_capacity_exhausted",
            ),
        };
        let mut response = (status, Json(ErrorDocument { error: code })).into_response();
        if self == Self::CapacityExceeded {
            response.headers_mut().insert(
                axum::http::header::RETRY_AFTER,
                HeaderValue::from_static("60"),
            );
        }
        response
    }
}

#[derive(Serialize)]
struct ErrorDocument {
    error: &'static str,
}
