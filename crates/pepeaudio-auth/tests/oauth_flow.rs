mod support;

use axum::{
    body::Body,
    http::{
        Method, Request, StatusCode,
        header::{CACHE_CONTROL, COOKIE, LOCATION, REFERRER_POLICY, RETRY_AFTER, SET_COOKIE},
    },
};
use http_body_util::BodyExt as _;
use pepeaudio_auth::build_auth_router;
use serde_json::Value;
use tower::ServiceExt as _;
use url::Url;

use support::{OLD_SESSION_TOKEN, SESSION_TOKEN, parts};

#[tokio::test]
async fn complete_flow_rotates_session_and_requires_csrf_logout() {
    let parts = parts();
    let oauth = parts.oauth.clone();
    let sessions = parts.sessions.clone();
    let app = build_auth_router(parts.service);

    let state = begin_login(&app).await;
    complete_callback(&app, &state, &oauth, &sessions).await;
    let csrf = read_session_and_guilds(&app).await;
    verify_logout(&app, &csrf, &sessions).await;
}

#[tokio::test]
async fn login_admission_exhaustion_is_retryable_and_does_not_set_state_cookie() {
    let parts = parts();
    parts.pending.exhaust_capacity();
    let app = build_auth_router(parts.service);

    let response = app
        .oneshot(request(Method::GET, "/auth/login", None, None))
        .await
        .expect("login response");

    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(response.headers()[RETRY_AFTER], "60");
    assert!(response.headers().get(SET_COOKIE).is_none());
    assert_eq!(
        json_body(response).await["error"],
        "authentication_capacity_exhausted"
    );
}

async fn begin_login(app: &axum::Router) -> String {
    let login = app
        .clone()
        .oneshot(request(Method::GET, "/auth/login", None, None))
        .await
        .expect("login response");
    assert_eq!(login.status(), StatusCode::SEE_OTHER);
    assert_eq!(login.headers()[CACHE_CONTROL], "private, no-store");
    assert_eq!(login.headers()[REFERRER_POLICY], "no-referrer");
    let authorize = Url::parse(
        login.headers()[LOCATION]
            .to_str()
            .expect("authorization URL"),
    )
    .expect("authorization URL");
    assert_eq!(authorize.scheme(), "https");
    assert_eq!(authorize.host_str(), Some("discord.com"));
    let query: std::collections::HashMap<_, _> = authorize.query_pairs().into_owned().collect();
    assert_eq!(
        query.get("scope").map(String::as_str),
        Some("identify guilds")
    );
    assert_eq!(
        query.get("code_challenge_method").map(String::as_str),
        Some("S256")
    );
    assert_eq!(
        query.get("redirect_uri").map(String::as_str),
        Some("https://audio.example.test/auth/callback")
    );
    let state = query.get("state").expect("state");
    assert_eq!(state.len(), 43);
    let state_cookie = find_cookie(&login, "__Host-pepeaudio_oauth_state");
    assert_eq!(state_cookie, state.as_str());
    state.clone()
}

async fn complete_callback(
    app: &axum::Router,
    state: &str,
    oauth: &support::FakeOAuth,
    sessions: &support::FakeSessions,
) {
    let wrong_cookie = "WWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWW";
    let wrong = app
        .clone()
        .oneshot(request(
            Method::GET,
            &format!("/auth/callback?code=code-123&state={state}"),
            Some(&format!("__Host-pepeaudio_oauth_state={wrong_cookie}")),
            None,
        ))
        .await
        .expect("mismatch response");
    assert_eq!(wrong.status(), StatusCode::BAD_REQUEST);
    assert!(oauth.calls.lock().expect("calls").is_empty());

    let callback_cookie = format!(
        "__Host-pepeaudio_oauth_state={state}; __Host-pepeaudio_session={OLD_SESSION_TOKEN}"
    );
    let callback = app
        .clone()
        .oneshot(request(
            Method::GET,
            &format!("/auth/callback?code=code-123&state={state}"),
            Some(&callback_cookie),
            None,
        ))
        .await
        .expect("callback response");
    assert_eq!(callback.status(), StatusCode::SEE_OTHER);
    assert_eq!(callback.headers()[LOCATION], "/app");
    assert_eq!(
        find_cookie(&callback, "__Host-pepeaudio_session"),
        SESSION_TOKEN
    );
    assert_ne!(SESSION_TOKEN, OLD_SESSION_TOKEN);
    assert!(sessions.contains(SESSION_TOKEN));
    {
        let calls = oauth.calls.lock().expect("calls");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "code-123");
        assert_eq!(calls[0].1.len(), 43);
    }
}

async fn read_session_and_guilds(app: &axum::Router) -> String {
    let session_cookie = format!("__Host-pepeaudio_session={SESSION_TOKEN}");
    let session = app
        .clone()
        .oneshot(request(
            Method::GET,
            "/auth/session",
            Some(&session_cookie),
            None,
        ))
        .await
        .expect("session response");
    assert_eq!(session.status(), StatusCode::OK);
    assert_eq!(session.headers()[CACHE_CONTROL], "private, no-store");
    let session_json = json_body(session).await;
    assert_eq!(session_json["userId"], "111");
    assert_eq!(session_json["username"], "pepe-listener");
    assert_eq!(session_json["displayName"], "Pepe Listener");
    assert_eq!(session_json["avatar"], "a_profilehash");
    let csrf = session_json["csrfToken"]
        .as_str()
        .expect("CSRF token")
        .to_owned();
    assert_eq!(csrf.len(), 43);
    let serialized = session_json.to_string();
    assert!(!serialized.contains(SESSION_TOKEN));
    assert!(!serialized.contains("access_token"));
    assert!(!serialized.contains("refresh_token"));

    let guilds = app
        .clone()
        .oneshot(request(
            Method::GET,
            "/auth/guilds",
            Some(&session_cookie),
            None,
        ))
        .await
        .expect("guild response");
    assert_eq!(guilds.headers()[CACHE_CONTROL], "private, no-store");
    let guild_json = json_body(guilds).await;
    assert_eq!(guild_json["guilds"][0]["id"], "222");
    assert_eq!(guild_json["guilds"][0]["permissions"], u64::MAX.to_string());
    assert_eq!(guild_json["guilds"][0]["botPresent"], true);
    assert_eq!(guild_json["guilds"][1]["botPresent"], false);
    csrf
}

async fn verify_logout(app: &axum::Router, csrf: &str, sessions: &support::FakeSessions) {
    let session_cookie = format!("__Host-pepeaudio_session={SESSION_TOKEN}");
    let get_logout = app
        .clone()
        .oneshot(request(
            Method::GET,
            "/auth/logout",
            Some(&session_cookie),
            None,
        ))
        .await
        .expect("GET logout");
    assert_eq!(get_logout.status(), StatusCode::METHOD_NOT_ALLOWED);

    let missing_csrf = app
        .clone()
        .oneshot(request(
            Method::POST,
            "/auth/logout",
            Some(&session_cookie),
            None,
        ))
        .await
        .expect("logout without CSRF");
    assert_eq!(missing_csrf.status(), StatusCode::FORBIDDEN);
    assert!(sessions.contains(SESSION_TOKEN));

    let logout = app
        .clone()
        .oneshot(request(
            Method::POST,
            "/auth/logout",
            Some(&session_cookie),
            Some(csrf),
        ))
        .await
        .expect("logout response");
    assert_eq!(logout.status(), StatusCode::NO_CONTENT);
    assert!(!sessions.contains(SESSION_TOKEN));
    assert_eq!(find_cookie(&logout, "__Host-pepeaudio_session"), "");
}

#[tokio::test]
async fn callback_rejects_open_redirect_parameters_before_exchange() {
    let parts = parts();
    let app = build_auth_router(parts.service);
    let login = app
        .clone()
        .oneshot(request(Method::GET, "/auth/login", None, None))
        .await
        .expect("login");
    let authorize = Url::parse(login.headers()[LOCATION].to_str().expect("location")).expect("URL");
    let state = authorize
        .query_pairs()
        .find_map(|(name, value)| (name == "state").then(|| value.into_owned()))
        .expect("state");
    let cookie = format!("__Host-pepeaudio_oauth_state={state}");
    let response = app
        .oneshot(request(
            Method::GET,
            &format!("/auth/callback?code=code-123&state={state}&next=https%3A%2F%2Fevil.test"),
            Some(&cookie),
            None,
        ))
        .await
        .expect("callback");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(response.headers()[CACHE_CONTROL], "private, no-store");
    assert!(parts.oauth.calls.lock().expect("calls").is_empty());
}

fn request(method: Method, uri: &str, cookie: Option<&str>, csrf: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(cookie) = cookie {
        builder = builder.header(COOKIE, cookie);
    }
    if let Some(csrf) = csrf {
        builder = builder.header("x-csrf-token", csrf);
    }
    builder.body(Body::empty()).expect("request")
}

fn find_cookie(response: &axum::response::Response, name: &str) -> String {
    response
        .headers()
        .get_all(SET_COOKIE)
        .iter()
        .filter_map(|header| header.to_str().ok())
        .find_map(|header| {
            header
                .split(';')
                .next()
                .and_then(|pair| pair.split_once('='))
                .and_then(|(cookie_name, value)| (cookie_name == name).then(|| value.to_owned()))
        })
        .expect("set-cookie")
}

async fn json_body(response: axum::response::Response) -> Value {
    let body = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    serde_json::from_slice(&body).expect("JSON body")
}
