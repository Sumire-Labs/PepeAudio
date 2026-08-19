use axum::http::{HeaderMap, header};
use pepeaudio_core::{GuildId, StateRevision, UnixTimeMillis};
use uuid::Uuid;

use crate::{ApiConfig, ApiError, Principal};

pub(crate) const IDEMPOTENCY_HEADER: &str = "idempotency-key";
pub(crate) const LAST_EVENT_ID_HEADER: &str = "last-event-id";
const FETCH_SITE_HEADER: &str = "sec-fetch-site";

pub(crate) fn guild_id(value: &str) -> Result<GuildId, ApiError> {
    value
        .parse()
        .map_err(|_| ApiError::BadRequest("invalid guild ID"))
}

pub(crate) fn validate_mutation_headers(
    headers: &HeaderMap,
    config: &ApiConfig,
    principal: &Principal,
) -> Result<Uuid, ApiError> {
    let origin = headers.get(header::ORIGIN).ok_or(ApiError::Forbidden)?;
    if origin.as_bytes() != config.allowed_origin().as_bytes() {
        return Err(ApiError::Forbidden);
    }
    if headers
        .get(FETCH_SITE_HEADER)
        .is_none_or(|value| value.as_bytes() != b"same-origin")
    {
        return Err(ApiError::Forbidden);
    }

    let supplied_csrf = headers
        .get(crate::CSRF_HEADER)
        .ok_or(ApiError::Forbidden)?
        .to_str()
        .map_err(|_| ApiError::Forbidden)?;
    if !constant_time_equal(supplied_csrf.as_bytes(), principal.csrf_token().as_bytes()) {
        return Err(ApiError::Forbidden);
    }

    let idempotency = headers
        .get(IDEMPOTENCY_HEADER)
        .ok_or(ApiError::BadRequest("idempotency-key header is required"))?
        .to_str()
        .map_err(|_| ApiError::BadRequest("idempotency-key must be a UUID"))?
        .parse::<Uuid>()
        .map_err(|_| ApiError::BadRequest("idempotency-key must be a UUID"))?;
    if idempotency.is_nil() {
        return Err(ApiError::BadRequest("idempotency-key must not be nil"));
    }
    Ok(idempotency)
}

pub(crate) fn command_deadline(
    now: UnixTimeMillis,
    config: &ApiConfig,
) -> Result<UnixTimeMillis, ApiError> {
    let ttl = u64::try_from(config.command_ttl().as_millis()).map_err(|_| ApiError::Internal)?;
    now.get()
        .checked_add(ttl)
        .map(UnixTimeMillis::new)
        .ok_or(ApiError::Internal)
}

pub(crate) fn last_event_id(headers: &HeaderMap) -> Result<Option<StateRevision>, ApiError> {
    headers
        .get(LAST_EVENT_ID_HEADER)
        .map(|value| {
            value
                .to_str()
                .map_err(|_| ApiError::BadRequest("last-event-id must be a revision"))?
                .parse::<u64>()
                .map(StateRevision::new)
                .map_err(|_| ApiError::BadRequest("last-event-id must be a revision"))
        })
        .transpose()
}

fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    let mut difference = left.len() ^ right.len();
    let shared = left.len().min(right.len());
    for index in 0..shared {
        difference |= usize::from(left[index] ^ right[index]);
    }
    difference == 0
}
