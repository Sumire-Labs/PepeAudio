mod support;

use axum::{
    body::Body,
    http::{Method, StatusCode, header},
};
use pepeaudio_api::{CSRF_HEADER, SnapshotSource};
use pepeaudio_core::{PlayerState, StateRevision};
use serde_json::json;
use tower::ServiceExt;
use uuid::Uuid;

use support::{CSRF, ORIGIN, fixture, json_body, mutation_request, request};

#[tokio::test]
async fn mutation_is_revisioned_and_idempotent_at_the_command_owner() {
    let fixture = fixture(4);
    let key = Uuid::from_u128(100);
    let pause = json!({
        "expected_revision": 7,
        "command": { "type": "pause" }
    });

    let first = fixture
        .app
        .clone()
        .oneshot(mutation_request(&fixture, key, &pause))
        .await
        .expect("router response");
    assert_eq!(first.status(), StatusCode::ACCEPTED);
    let first_body = json_body(first).await;
    assert_eq!(first_body["resulting_revision"], 8);
    assert_eq!(first_body["replayed"], false);

    let snapshot = fixture
        .backend
        .snapshot(fixture.guild_id)
        .await
        .expect("snapshot lookup")
        .expect("snapshot exists");
    assert_eq!(snapshot.revision, StateRevision::new(8));
    assert_eq!(snapshot.state, PlayerState::Paused);

    let replay = fixture
        .app
        .clone()
        .oneshot(mutation_request(&fixture, key, &pause))
        .await
        .expect("router response");
    assert_eq!(replay.status(), StatusCode::ACCEPTED);
    assert_eq!(json_body(replay).await["replayed"], true);

    let conflict = fixture
        .app
        .clone()
        .oneshot(mutation_request(
            &fixture,
            key,
            &json!({
                "expected_revision": 7,
                "command": { "type": "stop" }
            }),
        ))
        .await
        .expect("router response");
    assert_eq!(conflict.status(), StatusCode::CONFLICT);
    assert_eq!(
        json_body(conflict).await["error"]["code"],
        "idempotency_conflict"
    );

    let stale = fixture
        .app
        .clone()
        .oneshot(mutation_request(
            &fixture,
            Uuid::from_u128(101),
            &json!({
                "expected_revision": 7,
                "command": { "type": "stop" }
            }),
        ))
        .await
        .expect("router response");
    assert_eq!(stale.status(), StatusCode::CONFLICT);
    let stale_body = json_body(stale).await;
    assert_eq!(stale_body["error"]["code"], "revision_conflict");
    assert_eq!(stale_body["error"]["current_revision"], 8);

    let invalid = fixture
        .app
        .clone()
        .oneshot(mutation_request(
            &fixture,
            Uuid::from_u128(102),
            &json!({
                "expected_revision": 8,
                "command": { "type": "pause" }
            }),
        ))
        .await
        .expect("router response");
    assert_eq!(invalid.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        json_body(invalid).await["error"]["code"],
        "invalid_player_command"
    );
}

#[tokio::test]
async fn mutation_removes_one_upcoming_track_by_uuid() {
    let fixture = fixture(4);
    let removed = Uuid::from_u128(41);
    let response = fixture
        .app
        .clone()
        .oneshot(mutation_request(
            &fixture,
            Uuid::from_u128(110),
            &json!({
                "expected_revision": 7,
                "command": { "type": "remove_queued", "track_id": removed }
            }),
        ))
        .await
        .expect("router response");

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let snapshot = fixture
        .backend
        .snapshot(fixture.guild_id)
        .await
        .expect("snapshot lookup")
        .expect("snapshot exists");
    assert_eq!(snapshot.queued_tracks, 1);
    assert_eq!(snapshot.upcoming_tracks.len(), 1);
    assert_eq!(snapshot.upcoming_tracks[0].track_id, Uuid::from_u128(42));
}

#[tokio::test]
async fn mutation_reorders_upcoming_tracks_by_uuid() {
    let fixture = fixture(4);
    let response = fixture
        .app
        .clone()
        .oneshot(mutation_request(
            &fixture,
            Uuid::from_u128(111),
            &json!({
                "expected_revision": 7,
                "command": {
                    "type": "move_queued",
                    "track_id": Uuid::from_u128(42),
                    "before_track_id": Uuid::from_u128(41)
                }
            }),
        ))
        .await
        .expect("router response");

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let snapshot = fixture
        .backend
        .snapshot(fixture.guild_id)
        .await
        .expect("snapshot lookup")
        .expect("snapshot exists");
    assert_eq!(snapshot.revision, StateRevision::new(8));
    assert_eq!(snapshot.queued_tracks, 2);
    assert_eq!(
        snapshot
            .upcoming_tracks
            .iter()
            .map(|track| track.track_id)
            .collect::<Vec<_>>(),
        vec![Uuid::from_u128(42), Uuid::from_u128(41)]
    );
}

#[tokio::test]
async fn mutation_keeps_the_revision_for_an_already_satisfied_queue_move() {
    let fixture = fixture(4);
    let response = fixture
        .app
        .clone()
        .oneshot(mutation_request(
            &fixture,
            Uuid::from_u128(112),
            &json!({
                "expected_revision": 7,
                "command": {
                    "type": "move_queued",
                    "track_id": Uuid::from_u128(41),
                    "before_track_id": Uuid::from_u128(42)
                }
            }),
        ))
        .await
        .expect("router response");

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert_eq!(json_body(response).await["resulting_revision"], 7);
    let snapshot = fixture
        .backend
        .snapshot(fixture.guild_id)
        .await
        .expect("snapshot lookup")
        .expect("snapshot exists");
    assert_eq!(snapshot.revision, StateRevision::new(7));
    assert_eq!(
        snapshot
            .upcoming_tracks
            .iter()
            .map(|track| track.track_id)
            .collect::<Vec<_>>(),
        vec![Uuid::from_u128(41), Uuid::from_u128(42)]
    );
}

#[tokio::test]
async fn mutation_rejects_cross_site_browser_requests() {
    let fixture = fixture(4);
    let path = format!("/api/v1/guilds/{}/player/commands", fixture.guild_id);
    let body = json!({
        "expected_revision": 7,
        "command": { "type": "pause" }
    });
    let body_text = body.to_string();

    let missing_origin = fixture
        .app
        .clone()
        .oneshot(
            request(&fixture, Method::POST, &path)
                .header(header::CONTENT_TYPE, "application/json")
                .header("sec-fetch-site", "same-origin")
                .header(CSRF_HEADER, CSRF)
                .header("idempotency-key", Uuid::from_u128(200).to_string())
                .body(Body::from(body_text.clone()))
                .expect("valid request"),
        )
        .await
        .expect("router response");
    assert_eq!(missing_origin.status(), StatusCode::FORBIDDEN);

    let wrong_origin = fixture
        .app
        .clone()
        .oneshot(
            request(&fixture, Method::POST, &path)
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::ORIGIN, "https://attacker.example")
                .header("sec-fetch-site", "cross-site")
                .header(CSRF_HEADER, CSRF)
                .header("idempotency-key", Uuid::from_u128(203).to_string())
                .body(Body::from(body_text.clone()))
                .expect("valid request"),
        )
        .await
        .expect("router response");
    assert_eq!(wrong_origin.status(), StatusCode::FORBIDDEN);
    assert!(
        wrong_origin
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .is_none()
    );

    let missing_fetch_metadata = fixture
        .app
        .clone()
        .oneshot(
            request(&fixture, Method::POST, &path)
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::ORIGIN, ORIGIN)
                .header(CSRF_HEADER, CSRF)
                .header("idempotency-key", Uuid::from_u128(204).to_string())
                .body(Body::from(body_text.clone()))
                .expect("valid request"),
        )
        .await
        .expect("router response");
    assert_eq!(missing_fetch_metadata.status(), StatusCode::FORBIDDEN);

    let cross_site = fixture
        .app
        .clone()
        .oneshot(
            request(&fixture, Method::POST, &path)
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::ORIGIN, ORIGIN)
                .header("sec-fetch-site", "cross-site")
                .header(CSRF_HEADER, CSRF)
                .header("idempotency-key", Uuid::from_u128(205).to_string())
                .body(Body::from(body_text.clone()))
                .expect("valid request"),
        )
        .await
        .expect("router response");
    assert_eq!(cross_site.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn mutation_rejects_missing_csrf_and_bad_input() {
    let fixture = fixture(4);
    let path = format!("/api/v1/guilds/{}/player/commands", fixture.guild_id);
    let body = json!({
        "expected_revision": 7,
        "command": { "type": "pause" }
    });
    let body_text = body.to_string();

    let missing_csrf = fixture
        .app
        .clone()
        .oneshot(
            request(&fixture, Method::POST, &path)
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::ORIGIN, ORIGIN)
                .header("sec-fetch-site", "same-origin")
                .header("idempotency-key", Uuid::from_u128(201).to_string())
                .body(Body::from(body_text.clone()))
                .expect("valid request"),
        )
        .await
        .expect("router response");
    assert_eq!(missing_csrf.status(), StatusCode::FORBIDDEN);

    let bad_key = fixture
        .app
        .clone()
        .oneshot(
            request(&fixture, Method::POST, &path)
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::ORIGIN, ORIGIN)
                .header("sec-fetch-site", "same-origin")
                .header(CSRF_HEADER, CSRF)
                .header("idempotency-key", "not-a-uuid")
                .body(Body::from(body_text))
                .expect("valid request"),
        )
        .await
        .expect("router response");
    assert_eq!(bad_key.status(), StatusCode::BAD_REQUEST);

    let nil_key = fixture
        .app
        .clone()
        .oneshot(mutation_request(&fixture, Uuid::nil(), &body))
        .await
        .expect("router response");
    assert_eq!(nil_key.status(), StatusCode::BAD_REQUEST);

    let malformed_json = fixture
        .app
        .clone()
        .oneshot(
            request(&fixture, Method::POST, &path)
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::ORIGIN, ORIGIN)
                .header("sec-fetch-site", "same-origin")
                .header(CSRF_HEADER, CSRF)
                .header("idempotency-key", Uuid::from_u128(202).to_string())
                .body(Body::from("{"))
                .expect("valid request"),
        )
        .await
        .expect("router response");
    assert_eq!(malformed_json.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        json_body(malformed_json).await["error"]["code"],
        "invalid_request"
    );
}
