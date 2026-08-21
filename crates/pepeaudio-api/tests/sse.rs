mod support;

use std::time::Duration;

use axum::{
    body::Body,
    http::{Method, StatusCode, header},
};
use http_body_util::BodyExt;
use tokio::time::timeout;
use tower::ServiceExt;

use support::{empty_fixture, fixture, playing_snapshot, request};

#[tokio::test]
async fn sse_waits_from_revision_zero_before_the_first_player_snapshot() {
    let fixture = empty_fixture(4);
    let path = format!("/api/v1/guilds/{}/events", fixture.guild_id);
    let response = fixture
        .app
        .clone()
        .oneshot(
            request(&fixture, Method::GET, &path)
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("router response");
    assert_eq!(response.status(), StatusCode::OK);

    let mut body = response.into_body();
    let initial = timeout(Duration::from_secs(1), body.frame())
        .await
        .expect("initial SSE event timeout")
        .expect("initial SSE frame")
        .expect("initial body frame")
        .into_data()
        .expect("initial data frame");
    let initial = String::from_utf8(initial.to_vec()).expect("UTF-8 SSE frame");
    assert!(initial.contains("event: snapshot"));
    assert!(initial.contains("id: 0"));
    assert!(initial.contains("\"state\":\"disconnected\""));

    fixture
        .backend
        .publish_snapshot(playing_snapshot(fixture.guild_id, fixture.user_id, 1))
        .expect("publish first snapshot");
    let update = timeout(Duration::from_secs(1), body.frame())
        .await
        .expect("player SSE event timeout")
        .expect("player SSE frame")
        .expect("player body frame")
        .into_data()
        .expect("player data frame");
    let update = String::from_utf8(update.to_vec()).expect("UTF-8 SSE frame");
    assert!(update.contains("event: player"));
    assert!(update.contains("id: 1"));
}

#[tokio::test]
async fn sse_sends_a_full_snapshot_then_revisioned_player_events() {
    let fixture = fixture(4);
    let path = format!("/api/v1/guilds/{}/events", fixture.guild_id);
    let response = fixture
        .app
        .clone()
        .oneshot(
            request(&fixture, Method::GET, &path)
                .header("last-event-id", "6")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("router response");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "text/event-stream"
    );
    assert_eq!(
        response.headers()[header::CACHE_CONTROL],
        "private, no-store"
    );

    let mut body = response.into_body();
    let initial = timeout(Duration::from_secs(1), body.frame())
        .await
        .expect("initial SSE event timeout")
        .expect("initial SSE frame")
        .expect("initial body frame")
        .into_data()
        .expect("initial data frame");
    let initial = String::from_utf8(initial.to_vec()).expect("UTF-8 SSE frame");
    assert!(initial.contains("event: snapshot"));
    assert!(initial.contains("id: 7"));
    assert!(initial.contains("\"revision\":7"));

    fixture
        .backend
        .publish_snapshot(playing_snapshot(fixture.guild_id, fixture.user_id, 8))
        .expect("publish next snapshot");
    let update = timeout(Duration::from_secs(1), body.frame())
        .await
        .expect("player SSE event timeout")
        .expect("player SSE frame")
        .expect("player body frame")
        .into_data()
        .expect("player data frame");
    let update = String::from_utf8(update.to_vec()).expect("UTF-8 SSE frame");
    assert!(update.contains("event: player"));
    assert!(update.contains("id: 8"));
    assert!(update.contains("\"revision\":8"));
}

#[tokio::test]
async fn per_user_sse_admission_returns_429_and_releases_on_body_drop() {
    let fixture = fixture(16);
    let path = format!("/api/v1/guilds/{}/events", fixture.guild_id);
    let mut established = Vec::new();
    for _ in 0..8 {
        let response = fixture
            .app
            .clone()
            .oneshot(
                request(&fixture, Method::GET, &path)
                    .body(Body::empty())
                    .expect("valid request"),
            )
            .await
            .expect("router response");
        assert_eq!(response.status(), StatusCode::OK);
        established.push(response);
    }

    let rejected = fixture
        .app
        .clone()
        .oneshot(
            request(&fixture, Method::GET, &path)
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("router response");
    assert_eq!(rejected.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(rejected.headers()[header::RETRY_AFTER], "5");

    drop(established.pop());
    let admitted = fixture
        .app
        .clone()
        .oneshot(
            request(&fixture, Method::GET, &path)
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("router response after release");
    assert_eq!(admitted.status(), StatusCode::OK);
}

#[tokio::test]
async fn bounded_sse_lag_emits_resync_and_closes() {
    let fixture = fixture(1);
    let path = format!("/api/v1/guilds/{}/events", fixture.guild_id);
    let response = fixture
        .app
        .clone()
        .oneshot(
            request(&fixture, Method::GET, &path)
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("router response");

    for revision in 8..=10 {
        fixture
            .backend
            .publish_snapshot(playing_snapshot(
                fixture.guild_id,
                fixture.user_id,
                revision,
            ))
            .expect("publish snapshot");
    }

    let bytes = timeout(Duration::from_secs(1), response.into_body().collect())
        .await
        .expect("lagged stream should close")
        .expect("SSE body")
        .to_bytes();
    let body = String::from_utf8(bytes.to_vec()).expect("UTF-8 SSE body");
    assert!(body.contains("event: snapshot"));
    assert!(body.contains("event: resync"));
    assert!(body.contains("bounded_lag"));
    assert!(body.contains("\"last_revision\":7"));
}

#[tokio::test]
async fn malformed_last_event_id_is_rejected_before_streaming() {
    let fixture = fixture(4);
    let path = format!("/api/v1/guilds/{}/events", fixture.guild_id);
    let response = fixture
        .app
        .clone()
        .oneshot(
            request(&fixture, Method::GET, &path)
                .header("last-event-id", "not-a-revision")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("router response");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn api_shutdown_finishes_an_established_sse_response() {
    let fixture = fixture(4);
    let path = format!("/api/v1/guilds/{}/events", fixture.guild_id);
    let response = fixture
        .app
        .clone()
        .oneshot(
            request(&fixture, Method::GET, &path)
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("router response");
    let mut body = response.into_body();
    let _initial = timeout(Duration::from_secs(1), body.frame())
        .await
        .expect("initial SSE event timeout")
        .expect("initial SSE frame")
        .expect("initial body frame");

    fixture.shutdown.trigger();
    let remaining = timeout(Duration::from_secs(1), body.collect())
        .await
        .expect("shutdown should finish the SSE body")
        .expect("SSE body should close cleanly")
        .to_bytes();
    assert!(remaining.is_empty());
}
