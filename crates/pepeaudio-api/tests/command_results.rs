mod support;

use axum::{
    body::Body,
    http::{Method, StatusCode},
};
use pepeaudio_core::CommandResult;
use serde_json::json;
use tower::ServiceExt;
use uuid::Uuid;

use support::{fixture, json_body, mutation_request, request};

#[tokio::test]
async fn accepted_receipt_correlates_with_its_applied_result() {
    let fixture = fixture(4);
    let response = fixture
        .app
        .clone()
        .oneshot(mutation_request(
            &fixture,
            Uuid::from_u128(99),
            &json!({
                "expected_revision": 7,
                "command": { "type": "pause" }
            }),
        ))
        .await
        .expect("router response");
    let receipt = json_body(response).await;
    let command_id = receipt["command_id"]
        .as_str()
        .expect("command ID in receipt");

    let result = get_result(&fixture, &fixture.guild_id.to_string(), command_id).await;
    assert_eq!(result.status(), StatusCode::OK);
    let body = json_body(result).await;
    assert_eq!(body["command_id"], command_id);
    assert_eq!(body["guild_id"], fixture.guild_id.to_string());
    assert_eq!(body["status"], "applied");
    assert_eq!(body["resulting_revision"], 8);

    let other_guild = get_result(&fixture, "999", command_id).await;
    assert_eq!(other_guild.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn pending_and_expired_results_have_distinct_http_statuses() {
    let fixture = fixture(4);
    let pending_id = Uuid::new_v4();
    fixture
        .backend
        .publish_command_result(CommandResult::pending(pending_id, fixture.guild_id))
        .expect("seed pending result");

    let pending = get_result(
        &fixture,
        &fixture.guild_id.to_string(),
        &pending_id.to_string(),
    )
    .await;
    assert_eq!(pending.status(), StatusCode::ACCEPTED);
    assert_eq!(json_body(pending).await["status"], "pending");

    let missing_id = Uuid::new_v4();
    let missing = get_result(
        &fixture,
        &fixture.guild_id.to_string(),
        &missing_id.to_string(),
    )
    .await;
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        json_body(missing).await["error"]["code"],
        "command_result_not_found"
    );
}

#[tokio::test]
async fn command_result_rejects_malformed_identifiers() {
    let fixture = fixture(4);
    let malformed = get_result(&fixture, &fixture.guild_id.to_string(), "not-a-uuid").await;
    assert_eq!(malformed.status(), StatusCode::BAD_REQUEST);
}

async fn get_result(
    fixture: &support::Fixture,
    guild_id: &str,
    command_id: &str,
) -> axum::response::Response {
    fixture
        .app
        .clone()
        .oneshot(
            request(
                fixture,
                Method::GET,
                &format!("/api/v1/guilds/{guild_id}/player/commands/{command_id}"),
            )
            .body(Body::empty())
            .expect("valid request"),
        )
        .await
        .expect("router response")
}
