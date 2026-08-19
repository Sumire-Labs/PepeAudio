use super::*;
use crate::{http::tests::ScriptedTransport, parse_catalog_url};

fn json(status: u16, value: &serde_json::Value) -> HttpResponse {
    HttpResponse::new(status, serde_json::to_vec(&value).expect("JSON fixture"))
}

fn reference(value: &str) -> CatalogReference {
    parse_catalog_url(&Url::parse(value).expect("URL")).expect("catalog reference")
}

#[tokio::test]
async fn resolves_track_and_reuses_client_credentials_token() {
    let transport = ScriptedTransport::new([
        json(
            200,
            &serde_json::json!({
                "access_token": "access-token",
                "token_type": "Bearer",
                "expires_in": 3600
            }),
        ),
        json(
            200,
            &serde_json::json!({
                "id": "4uLU6hMCjMI75M1A2tKUQC",
                "name": "Never Gonna Give You Up",
                "artists": [{"name": "Rick Astley"}],
                "album": {"name": "Whenever You Need Somebody"},
                "duration_ms": 213_573,
                "external_ids": {"isrc": "GB-ARL-87-00110"},
                "type": "track",
                "is_local": false
            }),
        ),
        json(
            200,
            &serde_json::json!({
                "id": "4uLU6hMCjMI75M1A2tKUQC",
                "name": "Never Gonna Give You Up",
                "artists": [{"name": "Rick Astley"}],
                "album": {"name": "Whenever You Need Somebody"},
                "duration_ms": 213_573,
                "external_ids": {"isrc": "GBARL8700110"},
                "type": "track"
            }),
        ),
    ]);
    let client =
        SpotifyCatalog::with_transport("client-id", "client-secret", "JP", transport.clone())
            .expect("client");
    let reference = reference("https://open.spotify.com/track/4uLU6hMCjMI75M1A2tKUQC");

    let first = client.resolve(&reference, 25).await.expect("first result");
    let second = client.resolve(&reference, 25).await.expect("second result");

    assert_eq!(first.tracks[0].isrc.as_deref(), Some("GBARL8700110"));
    assert_eq!(second.tracks[0].title, "Never Gonna Give You Up");
    assert_eq!(transport.request_urls().len(), 3);
    assert_eq!(
        transport.request_header(1, "authorization").as_deref(),
        Some("Bearer access-token")
    );
}

#[tokio::test]
async fn surfaces_current_playlist_access_denial() {
    let transport = ScriptedTransport::new(Vec::<HttpResponse>::new());
    let client =
        SpotifyCatalog::with_transport("client-id", "client-secret", "JP", transport.clone())
            .expect("client");
    let reference = reference("https://open.spotify.com/playlist/37i9dQZF1DXcBWIGoYBM5M");

    assert_eq!(
        client.resolve(&reference, 25).await,
        Err(CatalogError::SpotifyPlaylistAccessDenied)
    );
    assert!(transport.request_urls().is_empty());
}

#[tokio::test]
async fn paginates_album_without_exceeding_collection_limit() {
    let album_id = "4aawyAB9vmqN3uQ7FjRGTy";
    let transport = ScriptedTransport::new([
        json(
            200,
            &serde_json::json!({
                "access_token": "access-token",
                "token_type": "Bearer",
                "expires_in": 3600
            }),
        ),
        json(
            200,
            &serde_json::json!({
                "name": "Example Album",
                "tracks": {
                    "items": [{
                        "id": "1111111111111111111111",
                        "name": "First",
                        "artists": [{"name": "Artist"}],
                        "duration_ms": 180_000,
                        "type": "track"
                    }],
                    "total": 3,
                    "next": "https://api.spotify.com/v1/albums/4aawyAB9vmqN3uQ7FjRGTy/tracks?offset=1"
                }
            }),
        ),
        json(
            200,
            &serde_json::json!({
                "items": [
                    {
                        "id": "2222222222222222222222",
                        "name": "Second",
                        "artists": [{"name": "Artist"}],
                        "duration_ms": 181_000,
                        "type": "track"
                    },
                    {
                        "id": "3333333333333333333333",
                        "name": "Third",
                        "artists": [{"name": "Artist"}],
                        "duration_ms": 182_000,
                        "type": "track"
                    }
                ],
                "total": 3,
                "next": null
            }),
        ),
    ]);
    let client =
        SpotifyCatalog::with_transport("client-id", "client-secret", "JP", transport.clone())
            .expect("client");
    let source = reference(&format!("https://open.spotify.com/album/{album_id}"));

    let collection = client.resolve(&source, 2).await.expect("album metadata");

    assert_eq!(collection.tracks.len(), 2);
    assert_eq!(collection.source_item_count, Some(3));
    assert!(collection.truncated);
    assert!(transport.request_urls()[2].contains("offset=1"));
}

#[tokio::test]
async fn unknown_album_total_remains_unknown_and_next_marks_truncation() {
    let item = |id: &str, name: &str| {
        serde_json::json!({
            "id": id,
            "name": name,
            "artists": [{"name": "Artist"}],
            "type": "track"
        })
    };
    let transport = ScriptedTransport::new([
        json(
            200,
            &serde_json::json!({
                "access_token": "access-token",
                "token_type": "Bearer",
                "expires_in": 3600
            }),
        ),
        json(
            200,
            &serde_json::json!({
                "name": "Unknown Size",
                "tracks": {
                    "items": [
                        item("1111111111111111111111", "First"),
                        item("2222222222222222222222", "Second")
                    ],
                    "next": "https://api.spotify.com/v1/albums/4aawyAB9vmqN3uQ7FjRGTy/tracks?offset=2"
                }
            }),
        ),
    ]);
    let client = SpotifyCatalog::with_transport("client-id", "client-secret", "JP", transport)
        .expect("client");
    let source = reference("https://open.spotify.com/album/4aawyAB9vmqN3uQ7FjRGTy");

    let collection = client.resolve(&source, 1).await.expect("album metadata");

    assert_eq!(collection.tracks.len(), 1);
    assert_eq!(collection.source_item_count, None);
    assert!(collection.truncated);
}
