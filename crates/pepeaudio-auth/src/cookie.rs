use std::time::Duration;

use axum::http::{HeaderMap, HeaderValue, header::COOKIE};
use pepeaudio_api::{CSRF_HEADER, SESSION_COOKIE};

use crate::crypto::constant_time_eq;

pub(crate) const OAUTH_STATE_COOKIE: &str = "__Host-pepeaudio_oauth_state";

pub(crate) fn session_cookie(headers: &HeaderMap) -> Result<Option<&str>, CookieError> {
    canonical_cookie(headers, SESSION_COOKIE)
}

pub(crate) fn oauth_state_cookie(headers: &HeaderMap) -> Result<Option<&str>, CookieError> {
    canonical_cookie(headers, OAUTH_STATE_COOKIE)
}

pub(crate) fn validate_csrf(headers: &HeaderMap, expected: &str) -> bool {
    headers
        .get(CSRF_HEADER)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|supplied| {
            supplied.len() == expected.len() && constant_time_eq(supplied, expected)
        })
}

pub(crate) fn session_set_cookie(token: &str, absolute_ttl: Duration) -> HeaderValue {
    secure_cookie(SESSION_COOKIE, token, absolute_ttl.as_secs())
}

pub(crate) fn state_set_cookie(state: &str, ttl: Duration) -> HeaderValue {
    secure_cookie(OAUTH_STATE_COOKIE, state, ttl.as_secs())
}

pub(crate) fn clear_session_cookie() -> HeaderValue {
    clear_cookie(SESSION_COOKIE)
}

pub(crate) fn clear_state_cookie() -> HeaderValue {
    clear_cookie(OAUTH_STATE_COOKIE)
}

fn canonical_cookie<'a>(
    headers: &'a HeaderMap,
    expected_name: &str,
) -> Result<Option<&'a str>, CookieError> {
    let mut found = None;
    for line in headers.get_all(COOKIE) {
        let line = line.to_str().map_err(|_| CookieError::Malformed)?;
        for pair in line.split(';') {
            let Some((name, value)) = pair.trim().split_once('=') else {
                continue;
            };
            if name != expected_name {
                continue;
            }
            if found.is_some() || value.len() != 43 || !is_token_shape(value) {
                return Err(CookieError::Malformed);
            }
            found = Some(value);
        }
    }
    Ok(found)
}

fn secure_cookie(name: &str, value: &str, max_age: u64) -> HeaderValue {
    debug_assert!(value.len() == 43 && is_token_shape(value));
    HeaderValue::from_str(&format!(
        "{name}={value}; Path=/; Max-Age={max_age}; Secure; HttpOnly; SameSite=Lax"
    ))
    .expect("validated cookie token and fixed attributes")
}

fn clear_cookie(name: &str) -> HeaderValue {
    HeaderValue::from_str(&format!(
        "{name}=; Path=/; Max-Age=0; Secure; HttpOnly; SameSite=Lax"
    ))
    .expect("fixed cookie attributes")
}

fn is_token_shape(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CookieError {
    Malformed,
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderMap, HeaderValue, header::COOKIE};

    use super::{clear_session_cookie, session_cookie, session_set_cookie};

    const TOKEN: &str = "abcdefghijklmnopqrstuvwxyz0123456789_-ABCDE";

    #[test]
    fn canonical_cookie_rejects_duplicates() {
        let mut headers = HeaderMap::new();
        headers.append(
            COOKIE,
            HeaderValue::from_str(&format!("__Host-pepeaudio_session={TOKEN}")).expect("cookie"),
        );
        headers.append(
            COOKIE,
            HeaderValue::from_str(&format!("__Host-pepeaudio_session={TOKEN}")).expect("cookie"),
        );
        assert!(session_cookie(&headers).is_err());
    }

    #[test]
    fn cookies_have_host_prefix_security_attributes() {
        let set = session_set_cookie(TOKEN, std::time::Duration::from_mins(1));
        let value = set.to_str().expect("header");
        assert!(value.contains("Path=/"));
        assert!(value.contains("Secure"));
        assert!(value.contains("HttpOnly"));
        assert!(value.contains("SameSite=Lax"));
        assert!(!value.contains("Domain="));
        assert!(
            clear_session_cookie()
                .to_str()
                .expect("header")
                .contains("Max-Age=0")
        );
    }
}
