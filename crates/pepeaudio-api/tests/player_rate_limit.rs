mod support;

use axum::http::{HeaderValue, StatusCode, header};
use pepeaudio_api::SnapshotSource;
use pepeaudio_core::StateRevision;
use serde_json::json;
use tower::ServiceExt;
use uuid::Uuid;

use support::{fixture, json_body, mutation_request};

#[tokio::test]
async fn authenticated_player_command_flood_returns_retry_after_without_mutation() {
    let fixture = fixture(4);

    for offset in 0_u64..20 {
        let body = json!({
            "expected_revision": 7 + offset,
            "command": { "type": "set_volume", "volume": 75 }
        });
        let mut request =
            mutation_request(&fixture, Uuid::from_u128(1_000 + u128::from(offset)), &body);
        request.headers_mut().insert(
            "x-forwarded-for",
            HeaderValue::from_str(&format!("203.0.113.{}", offset + 1))
                .expect("test address header"),
        );

        let response = fixture
            .app
            .clone()
            .oneshot(request)
            .await
            .expect("router response");
        assert_eq!(response.status(), StatusCode::ACCEPTED);
    }

    let body = json!({
        "expected_revision": 27,
        "command": { "type": "set_volume", "volume": 75 }
    });
    let mut rejected_request = mutation_request(&fixture, Uuid::from_u128(2_000), &body);
    rejected_request.headers_mut().insert(
        "x-forwarded-for",
        HeaderValue::from_static("198.51.100.200"),
    );
    let response = fixture
        .app
        .clone()
        .oneshot(rejected_request)
        .await
        .expect("router response");

    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        response.headers().get(header::RETRY_AFTER),
        Some(&HeaderValue::from_static("59"))
    );
    let body = json_body(response).await;
    assert_eq!(body["error"]["code"], "player_command_rate_limited");
    assert!(body["error"]["message"].as_str().is_some());

    let snapshot = fixture
        .backend
        .snapshot(fixture.guild_id)
        .await
        .expect("snapshot lookup")
        .expect("snapshot exists");
    assert_eq!(snapshot.revision, StateRevision::new(27));
}

#[tokio::test]
async fn idempotent_http_retries_count_as_admission_attempts() {
    let fixture = fixture(4);
    let replay_key = Uuid::from_u128(3_000);
    let first_body = json!({
        "expected_revision": 7,
        "command": { "type": "set_volume", "volume": 75 }
    });
    let first = fixture
        .app
        .clone()
        .oneshot(mutation_request(&fixture, replay_key, &first_body))
        .await
        .expect("first router response");
    assert_eq!(first.status(), StatusCode::ACCEPTED);

    for _ in 0..19 {
        let replay = fixture
            .app
            .clone()
            .oneshot(mutation_request(&fixture, replay_key, &first_body))
            .await
            .expect("replay router response");
        assert_eq!(replay.status(), StatusCode::ACCEPTED);
        assert_eq!(json_body(replay).await["replayed"], true);
    }

    let rejected = fixture
        .app
        .clone()
        .oneshot(mutation_request(&fixture, replay_key, &first_body))
        .await
        .expect("limited router response");
    assert_eq!(rejected.status(), StatusCode::TOO_MANY_REQUESTS);
}
