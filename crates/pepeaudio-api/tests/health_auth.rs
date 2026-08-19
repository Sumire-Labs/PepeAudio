mod support;

use axum::{
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use serde_json::json;
use tower::ServiceExt;

use support::{fixture, json_body, request};

#[tokio::test]
async fn health_distinguishes_liveness_from_dependency_readiness() {
    let fixture = fixture(4);
    let live = fixture
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/health/live")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("router response");
    assert_eq!(live.status(), StatusCode::OK);
    assert_eq!(json_body(live).await, json!({ "status": "live" }));

    fixture.backend.set_ready(false);
    let ready = fixture
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/health/ready")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("router response");
    assert_eq!(ready.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        json_body(ready).await["error"]["code"],
        "service_unavailable"
    );
}

#[tokio::test]
async fn player_snapshot_requires_authentication_and_current_authorization() {
    let fixture = fixture(4);
    let path = format!("/api/v1/guilds/{}/player", fixture.guild_id);

    let unauthenticated = fixture
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri(&path)
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("router response");
    assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);

    let forbidden = fixture
        .app
        .clone()
        .oneshot(
            request(&fixture, Method::GET, "/api/v1/guilds/999/player")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("router response");
    assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);

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
    assert_eq!(
        response.headers()[header::CACHE_CONTROL],
        "private, no-store"
    );
    assert!(
        response
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .is_none()
    );
    let body = json_body(response).await;
    assert_eq!(body["guild_id"], "10");
    assert_eq!(body["revision"], 7);
    assert_eq!(body["state"], "playing");
}
