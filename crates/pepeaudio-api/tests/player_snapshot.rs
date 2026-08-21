mod support;

use axum::{
    body::Body,
    http::{Method, StatusCode},
};
use tower::ServiceExt;

use support::{empty_fixture, json_body, request};

#[tokio::test]
async fn authorized_guild_without_a_durable_snapshot_is_disconnected() {
    let fixture = empty_fixture(4);
    let path = format!("/api/v1/guilds/{}/player", fixture.guild_id);
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
    let body = json_body(response).await;
    assert_eq!(body["guild_id"], fixture.guild_id.to_string());
    assert_eq!(body["revision"], 0);
    assert_eq!(body["state"], "disconnected");
    assert_eq!(body["volume"], 10);
    assert_eq!(body["queued_tracks"], 0);
    assert_eq!(body["observed_at"], 1_500);
}
