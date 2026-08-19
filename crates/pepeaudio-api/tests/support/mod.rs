#![allow(dead_code)]

use std::{sync::Arc, time::Duration};

use axum::{
    Router,
    body::Body,
    http::{Method, Request, Response, header},
};
use http_body_util::BodyExt;
use pepeaudio_api::{
    ApiConfig, ApiShutdown, AppState, CSRF_HEADER, DEV_USER_HEADER, DevHeaderAuthenticator,
    HrirPresetCatalogSource, HrirPresetSummary, HrirSourceMetadata, build_router,
    dev::{AllowListAuthorizer, InMemoryPlayerBackend, ManualClock, StaticHrirPresetCatalog},
};
use pepeaudio_core::{
    ChannelId, GuildId, HrirPresetId, PlayerSnapshot, PlayerState, RepeatMode, StateRevision,
    TrackSnapshot, UnixTimeMillis, UserId, Volume,
};
use serde_json::Value;
use uuid::Uuid;

pub(crate) const ORIGIN: &str = "http://localhost:5173";
pub(crate) const CSRF: &str = "integration-test-csrf-token";

pub(crate) struct Fixture {
    pub(crate) app: Router,
    pub(crate) backend: Arc<InMemoryPlayerBackend>,
    pub(crate) shutdown: ApiShutdown,
    pub(crate) guild_id: GuildId,
    pub(crate) user_id: UserId,
}

pub(crate) fn fixture(event_capacity: usize) -> Fixture {
    fixture_for_guild(event_capacity, GuildId::new(10).expect("valid guild ID"))
}

pub(crate) fn fixture_for_guild(event_capacity: usize, guild_id: GuildId) -> Fixture {
    let catalog = Arc::new(StaticHrirPresetCatalog::new([HrirPresetSummary {
        id: HrirPresetId::new("fixture-neutral").expect("preset ID"),
        display_name: "Fixture Neutral".into(),
        source: HrirSourceMetadata {
            license_name: Some("CC0-1.0".into()),
            source_url: Some("https://example.test/hrir-source".into()),
            attribution: Some("API test fixture".into()),
        },
    }]));
    fixture_with_catalog(event_capacity, guild_id, catalog)
}

pub(crate) fn fixture_with_catalog(
    event_capacity: usize,
    guild_id: GuildId,
    catalog: Arc<dyn HrirPresetCatalogSource>,
) -> Fixture {
    let user_id = UserId::new(20).expect("valid user ID");
    let backend = Arc::new(InMemoryPlayerBackend::new(
        [playing_snapshot(guild_id, user_id, 7)],
        event_capacity,
    ));
    let authorizer = Arc::new(AllowListAuthorizer::new());
    authorizer
        .grant(user_id, guild_id)
        .expect("grant should succeed");
    let authenticator = Arc::new(
        DevHeaderAuthenticator::new(user_id, CSRF).expect("development auth configuration"),
    );
    let config = ApiConfig::new(ORIGIN, Duration::from_secs(10)).expect("API configuration");
    let shutdown = ApiShutdown::new();
    let state = AppState::new(
        config,
        authenticator,
        authorizer,
        backend.clone(),
        catalog,
        backend.clone(),
        backend.clone(),
        backend.clone(),
        backend.clone(),
        Arc::new(ManualClock::new(1_500)),
    )
    .with_shutdown(shutdown.clone());

    Fixture {
        app: build_router(state),
        backend,
        shutdown,
        guild_id,
        user_id,
    }
}

pub(crate) fn playing_snapshot(
    guild_id: GuildId,
    user_id: UserId,
    revision: u64,
) -> PlayerSnapshot {
    let upcoming_tracks = [41_u128, 42]
        .into_iter()
        .map(|track_id| TrackSnapshot {
            track_id: Uuid::from_u128(track_id),
            title: format!("Queued API track {track_id}"),
            artist: None,
            album: None,
            provenance: None,
            requester_user_id: Some(user_id),
            duration_ms: Some(120_000),
            position_ms: 0,
            seekable: true,
        })
        .collect();
    PlayerSnapshot {
        guild_id,
        voice_channel_id: Some(ChannelId::new(30).expect("valid channel ID")),
        revision: StateRevision::new(revision),
        state: PlayerState::Playing,
        current_track: Some(TrackSnapshot {
            track_id: Uuid::from_u128(40),
            title: "API contract track".to_owned(),
            artist: None,
            album: None,
            provenance: None,
            requester_user_id: Some(user_id),
            duration_ms: Some(180_000),
            position_ms: 30_000,
            seekable: true,
        }),
        queued_tracks: 2,
        upcoming_tracks,
        has_previous_track: true,
        volume: Volume::DEFAULT,
        repeat_mode: RepeatMode::Off,
        shuffle_enabled: false,
        hrir_preset: None,
        spatial_audio_enabled: false,
        observed_at: UnixTimeMillis::new(1_000),
    }
}

pub(crate) fn request(
    fixture: &Fixture,
    method: Method,
    path: &str,
) -> axum::http::request::Builder {
    Request::builder()
        .method(method)
        .uri(path)
        .header(DEV_USER_HEADER, fixture.user_id.to_string())
}

pub(crate) fn mutation_request(
    fixture: &Fixture,
    idempotency_key: Uuid,
    body: &Value,
) -> Request<Body> {
    request(
        fixture,
        Method::POST,
        &format!("/api/v1/guilds/{}/player/commands", fixture.guild_id),
    )
    .header(header::CONTENT_TYPE, "application/json")
    .header(header::ORIGIN, ORIGIN)
    .header("sec-fetch-site", "same-origin")
    .header(CSRF_HEADER, CSRF)
    .header("idempotency-key", idempotency_key.to_string())
    .body(Body::from(body.to_string()))
    .expect("valid request")
}

pub(crate) async fn json_body(response: Response<Body>) -> Value {
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body should be readable")
        .to_bytes();
    serde_json::from_slice(&bytes).expect("response should be JSON")
}
