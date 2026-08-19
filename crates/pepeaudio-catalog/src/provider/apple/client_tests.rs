use p256::{
    ecdsa::SigningKey,
    pkcs8::{EncodePrivateKey, LineEnding},
};

use super::*;
use crate::{
    http::{HttpResponse, tests::ScriptedTransport},
    parse_catalog_url,
};

fn test_private_key() -> String {
    SigningKey::from_slice(&[9; 32])
        .expect("test key")
        .to_pkcs8_pem(LineEnding::LF)
        .expect("test PEM")
        .to_string()
}

fn json(value: &serde_json::Value) -> HttpResponse {
    HttpResponse::new(200, serde_json::to_vec(&value).expect("JSON fixture"))
}

fn reference(value: &str) -> CatalogReference {
    parse_catalog_url(&Url::parse(value).expect("URL")).expect("catalog reference")
}

#[tokio::test]
async fn resolves_catalog_song_with_developer_token() {
    let transport = ScriptedTransport::new([json(&serde_json::json!({
        "data": [{
            "id": "1440833542",
            "type": "songs",
            "attributes": {
                "name": "Example Song",
                "artistName": "Example Artist",
                "albumName": "Example Album",
                "durationInMillis": 201_000,
                "isrc": "JP-ABC-12-34567",
                "url": "https://music.apple.com/jp/album/example/1440833098?i=1440833542"
            }
        }]
    }))]);
    let client = AppleMusicCatalog::with_transport(
        "ABCDEFGHIJ",
        "1234567890",
        &test_private_key(),
        transport.clone(),
    )
    .expect("client");
    let source = reference("https://music.apple.com/jp/album/example/1440833098?i=1440833542");

    let collection = client.resolve(&source, 25).await.expect("song metadata");

    assert_eq!(collection.tracks[0].title, "Example Song");
    assert_eq!(collection.tracks[0].isrc.as_deref(), Some("JPABC1234567"));
    assert!(
        transport
            .request_header(0, "authorization")
            .is_some_and(|header| header.starts_with("Bearer eyJ"))
    );
}

#[tokio::test]
async fn rejects_cross_origin_pagination_link() {
    let transport = ScriptedTransport::new([
        json(&serde_json::json!({
            "data": [{
                "id": "1440833098",
                "type": "albums",
                "attributes": {"name": "Example Album"}
            }]
        })),
        json(&serde_json::json!({
            "data": [{
                "id": "1440833542",
                "type": "songs",
                "attributes": {
                    "name": "Example Song",
                    "artistName": "Example Artist",
                    "url": "https://music.apple.com/jp/album/example/1440833098?i=1440833542"
                }
            }],
            "next": "https://attacker.invalid/v1/catalog/jp/albums/1440833098/tracks?offset=1"
        })),
    ]);
    let client = AppleMusicCatalog::with_transport(
        "ABCDEFGHIJ",
        "1234567890",
        &test_private_key(),
        transport.clone(),
    )
    .expect("client");
    let source = reference("https://music.apple.com/jp/album/example/1440833098");

    assert_eq!(
        client.resolve(&source, 25).await,
        Err(CatalogError::InvalidResponse(CatalogProvider::AppleMusic))
    );
}

#[tokio::test]
async fn resolves_catalog_playlist_and_skips_non_song_resources() {
    let transport = ScriptedTransport::new([
        json(&serde_json::json!({
            "data": [{
                "id": "pl.1234",
                "type": "playlists",
                "attributes": {
                    "name": "Catalog Playlist",
                    "lastModifiedDate": "2026-08-14T00:00:00Z"
                }
            }]
        })),
        json(&serde_json::json!({
            "data": [
                {
                    "id": "1440833542",
                    "type": "songs",
                    "attributes": {
                        "name": "Example Song",
                        "artistName": "Example Artist",
                        "durationInMillis": 201_000,
                        "url": "https://music.apple.com/jp/album/example/1440833098?i=1440833542"
                    }
                },
                {
                    "id": "1234567890",
                    "type": "music-videos",
                    "attributes": {
                        "name": "Example Video",
                        "artistName": "Example Artist",
                        "url": "https://music.apple.com/jp/music-video/example/1234567890"
                    }
                }
            ],
            "meta": {"total": 2}
        })),
    ]);
    let client = AppleMusicCatalog::with_transport(
        "ABCDEFGHIJ",
        "1234567890",
        &test_private_key(),
        transport.clone(),
    )
    .expect("client");
    let source = reference("https://music.apple.com/jp/playlist/example/pl.1234");

    let collection = client
        .resolve(&source, 25)
        .await
        .expect("playlist metadata");

    assert_eq!(collection.title, "Catalog Playlist");
    assert_eq!(collection.tracks.len(), 1);
    assert_eq!(collection.skipped_item_count, 1);
    assert_eq!(collection.source_item_count, Some(2));
    assert_eq!(collection.version.as_deref(), Some("2026-08-14T00:00:00Z"));
}

#[tokio::test]
async fn unknown_album_total_stays_unknown_when_a_next_page_exists() {
    let transport = ScriptedTransport::new([
        json(&serde_json::json!({
            "data": [{
                "id": "1440833098",
                "type": "albums",
                "attributes": {"name": "Unknown Size"}
            }]
        })),
        json(&serde_json::json!({
            "data": [{
                "id": "1440833542",
                "type": "songs",
                "attributes": {
                    "name": "Example Song",
                    "artistName": "Example Artist",
                    "url": "https://music.apple.com/jp/album/example/1440833098?i=1440833542"
                }
            }],
            "next": "/v1/catalog/jp/albums/1440833098/tracks?offset=1"
        })),
    ]);
    let client = AppleMusicCatalog::with_transport(
        "ABCDEFGHIJ",
        "1234567890",
        &test_private_key(),
        transport.clone(),
    )
    .expect("client");
    let source = reference("https://music.apple.com/jp/album/example/1440833098");

    let collection = client.resolve(&source, 1).await.expect("album metadata");

    assert_eq!(collection.tracks.len(), 1);
    assert_eq!(collection.source_item_count, None);
    assert!(collection.truncated);
    assert_eq!(
        transport.request_urls()[1],
        "https://api.music.apple.com/v1/catalog/jp/albums/1440833098/tracks?limit=1"
    );
}
