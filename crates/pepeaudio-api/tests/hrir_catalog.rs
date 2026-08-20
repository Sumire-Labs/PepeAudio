mod support;

use std::sync::Arc;

use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
};
use pepeaudio_api::{BoxPortFuture, HrirPresetCatalog, HrirPresetCatalogSource, PortError};
use pepeaudio_core::GuildId;
use serde_json::json;
use tower::ServiceExt;

use support::{fixture, fixture_for_guild, fixture_with_catalog, json_body, request};

#[tokio::test]
async fn catalog_requires_authentication_and_read_player_authorization() {
    let fixture = fixture(4);
    let path = format!("/api/v1/guilds/{}/hrir-presets", fixture.guild_id);

    let unauthenticated = fixture
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri(&path)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);

    let forbidden = fixture
        .app
        .clone()
        .oneshot(
            request(&fixture, Method::GET, "/api/v1/guilds/999/hrir-presets")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn catalog_exposes_only_public_metadata_and_string_snowflakes() {
    let guild_id = GuildId::new(18_446_744_073_709_551_615).expect("maximum snowflake");
    let fixture = fixture_for_guild(4, guild_id);
    let path = format!("/api/v1/guilds/{guild_id}/hrir-presets");
    let response = fixture
        .app
        .clone()
        .oneshot(
            request(&fixture, Method::GET, &path)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        json_body(response).await,
        json!({
            "guild_id": "18446744073709551615",
            "presets": [{
                "id": "fixture-neutral",
                "display_name": "Fixture Neutral",
                "description": "Compact API test HRIR",
                "source": {
                    "license_name": "CC0-1.0",
                    "source_url": "https://example.test/hrir-source",
                    "attribution": "API test fixture"
                }
            }]
        })
    );
}

#[tokio::test]
async fn catalog_rejects_an_adapter_guild_mismatch() {
    let requested = GuildId::new(10).expect("guild");
    let fixture = fixture_with_catalog(4, requested, Arc::new(MismatchedCatalog));
    let response = fixture
        .app
        .clone()
        .oneshot(
            request(&fixture, Method::GET, "/api/v1/guilds/10/hrir-presets")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(json_body(response).await["error"]["code"], "internal_error");
}

struct MismatchedCatalog;

impl HrirPresetCatalogSource for MismatchedCatalog {
    fn hrir_presets(&self, _: GuildId) -> BoxPortFuture<'_, Result<HrirPresetCatalog, PortError>> {
        Box::pin(async {
            Ok(HrirPresetCatalog {
                guild_id: GuildId::new(11).expect("other guild"),
                presets: Vec::new(),
            })
        })
    }
}
