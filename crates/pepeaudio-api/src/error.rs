use std::time::Duration;

use axum::{
    Json,
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use pepeaudio_core::StateRevision;
use serde::Serialize;

use crate::{AuthenticationError, AuthorizationError, PortError, RouteError};

/// Stable JSON error envelope.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ErrorDocument {
    pub error: ErrorBody,
}

/// Public error fields. Internal causes and credentials are never serialized.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ErrorBody {
    /// Stable machine-readable code.
    pub code: &'static str,
    /// Short client-actionable description.
    pub message: &'static str,
    /// Authoritative revision for optimistic-concurrency recovery.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_revision: Option<StateRevision>,
}

/// HTTP adapter rejection with centralized status and safe JSON mapping.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ApiError {
    BadRequest(&'static str),
    Unauthenticated,
    Forbidden,
    NotFound,
    CommandResultNotFound,
    RevisionConflict { actual: StateRevision },
    IdempotencyConflict,
    InvalidCommand,
    CommandRateLimited { retry_after: Duration },
    Unavailable,
    SseCapacityExceeded,
    Internal,
}

impl ApiError {
    pub(crate) const fn invalid_request() -> Self {
        Self::BadRequest("request validation failed")
    }

    fn response_parts(&self) -> (StatusCode, ErrorBody) {
        match self {
            Self::BadRequest(message) => (
                StatusCode::BAD_REQUEST,
                ErrorBody {
                    code: "invalid_request",
                    message,
                    current_revision: None,
                },
            ),
            Self::Unauthenticated => (
                StatusCode::UNAUTHORIZED,
                ErrorBody {
                    code: "authentication_required",
                    message: "authentication is required",
                    current_revision: None,
                },
            ),
            Self::Forbidden => (
                StatusCode::FORBIDDEN,
                ErrorBody {
                    code: "forbidden",
                    message: "the operation is not permitted",
                    current_revision: None,
                },
            ),
            Self::NotFound => (
                StatusCode::NOT_FOUND,
                ErrorBody {
                    code: "player_not_found",
                    message: "the guild player was not found",
                    current_revision: None,
                },
            ),
            Self::CommandResultNotFound => (
                StatusCode::NOT_FOUND,
                ErrorBody {
                    code: "command_result_not_found",
                    message: "the command result expired or was not found",
                    current_revision: None,
                },
            ),
            Self::RevisionConflict { actual } => (
                StatusCode::CONFLICT,
                ErrorBody {
                    code: "revision_conflict",
                    message: "the player state changed; fetch a fresh snapshot",
                    current_revision: Some(*actual),
                },
            ),
            Self::IdempotencyConflict => (
                StatusCode::CONFLICT,
                ErrorBody {
                    code: "idempotency_conflict",
                    message: "the idempotency key was already used for another request",
                    current_revision: None,
                },
            ),
            Self::InvalidCommand => (
                StatusCode::UNPROCESSABLE_ENTITY,
                ErrorBody {
                    code: "invalid_player_command",
                    message: "the player command is invalid for the current state",
                    current_revision: None,
                },
            ),
            Self::CommandRateLimited { .. } => (
                StatusCode::TOO_MANY_REQUESTS,
                ErrorBody {
                    code: "player_command_rate_limited",
                    message: "too many player commands were submitted; retry later",
                    current_revision: None,
                },
            ),
            Self::Unavailable => (
                StatusCode::SERVICE_UNAVAILABLE,
                ErrorBody {
                    code: "service_unavailable",
                    message: "a required service is temporarily unavailable",
                    current_revision: None,
                },
            ),
            Self::SseCapacityExceeded => (
                StatusCode::TOO_MANY_REQUESTS,
                ErrorBody {
                    code: "sse_capacity_exhausted",
                    message: "too many live event streams are open; retry later",
                    current_revision: None,
                },
            ),
            Self::Internal => (
                StatusCode::INTERNAL_SERVER_ERROR,
                ErrorBody {
                    code: "internal_error",
                    message: "the request could not be completed",
                    current_revision: None,
                },
            ),
        }
    }

    fn retry_after_seconds(&self) -> Option<u64> {
        match self {
            Self::CommandRateLimited { retry_after } => {
                let millis = retry_after.as_millis();
                let rounded_up = millis.saturating_add(999) / 1_000;
                Some(u64::try_from(rounded_up.clamp(1, 60)).unwrap_or(60))
            }
            Self::SseCapacityExceeded => Some(5),
            _ => None,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let retry_after = self.retry_after_seconds();
        let (status, error) = self.response_parts();
        let mut response = (status, Json(ErrorDocument { error })).into_response();
        if let Some(seconds) = retry_after
            && let Ok(value) = HeaderValue::from_str(&seconds.to_string())
        {
            response.headers_mut().insert(header::RETRY_AFTER, value);
        }
        response
    }
}

impl From<AuthenticationError> for ApiError {
    fn from(value: AuthenticationError) -> Self {
        match value {
            AuthenticationError::Unauthenticated => Self::Unauthenticated,
            AuthenticationError::Unavailable => Self::Unavailable,
        }
    }
}

impl From<AuthorizationError> for ApiError {
    fn from(value: AuthorizationError) -> Self {
        match value {
            AuthorizationError::Forbidden => Self::Forbidden,
            AuthorizationError::Unavailable => Self::Unavailable,
        }
    }
}

impl From<PortError> for ApiError {
    fn from(value: PortError) -> Self {
        match value {
            PortError::NotFound => Self::NotFound,
            PortError::Unavailable => Self::Unavailable,
            PortError::Internal => Self::Internal,
        }
    }
}

impl From<RouteError> for ApiError {
    fn from(value: RouteError) -> Self {
        match value {
            RouteError::NotFound => Self::NotFound,
            RouteError::RevisionConflict { actual, .. } => Self::RevisionConflict { actual },
            RouteError::InvalidCommand => Self::InvalidCommand,
            RouteError::IdempotencyConflict => Self::IdempotencyConflict,
            RouteError::RateLimited { retry_after } => Self::CommandRateLimited { retry_after },
            RouteError::Unavailable => Self::Unavailable,
            RouteError::Internal => Self::Internal,
        }
    }
}
