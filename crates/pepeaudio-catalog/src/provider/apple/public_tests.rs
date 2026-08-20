use super::*;
use crate::{
    http::{HttpResponse, tests::ScriptedTransport},
    parse_catalog_url,
};

fn json(status: u16, value: &serde_json::Value) -> HttpResponse {
    HttpResponse::new(status, serde_json::to_vec(value).expect("JSON fixture"))
}

fn reference(value: &str) -> CatalogReference {
    parse_catalog_url(&Url::parse(value).expect("URL")).expect("catalog reference")
}

#[tokio::test]
async fn resolves_song_through_fixed_keyless_lookup_endpoint() {
    let transport = ScriptedTransport::new([json(
        200,
        &serde_json::json!({
            "resultCount": 1,
            "results": [{
                "wrapperType": "track",
                "kind": "song",
                "trackId": 1_440_833_542_u64,
                "collectionId": 1_440_833_098_u64,
                "trackName": "Example Song",
                "artistName": "Example Artist",
                "collectionName": "Example Album",
                "trackTimeMillis": 201_000,
                "previewUrl": "https://audio-ssl.itunes.apple.com/preview.m4a"
            }]
        }),
    )]);
    let client = AppleMusicPublicCatalog::with_transport(transport.clone());
    let source = reference("https://music.apple.com/jp/song/example/1440833542");

    let collection = client.resolve(&source, 25).await.expect("song metadata");

    assert_eq!(collection.title, "Example Song");
    assert_eq!(collection.tracks[0].artists, ["Example Artist"]);
    assert_eq!(collection.tracks[0].album.as_deref(), Some("Example Album"));
    assert_eq!(collection.tracks[0].duration_ms, Some(201_000));
    assert_eq!(collection.tracks[0].reference, source);
    assert_eq!(
        transport.request_urls(),
        ["https://itunes.apple.com/lookup?id=1440833542&country=JP"]
    );
    assert_eq!(
        transport.request_header(0, "accept").as_deref(),
        Some("application/json")
    );
    assert_eq!(
        transport.request_response_limit(0),
        Some(MAX_PUBLIC_RESPONSE_BYTES)
    );
}

#[tokio::test]
async fn resolves_album_tracks_and_skips_unrelated_resources() {
    let transport = ScriptedTransport::new([json(
        200,
        &serde_json::json!({
            "resultCount": 3,
            "results": [
                {
                    "wrapperType": "collection",
                    "collectionType": "Album",
                    "collectionId": 1_440_833_098_u64,
                    "collectionName": "Example Album"
                },
                {
                    "wrapperType": "track",
                    "kind": "song",
                    "trackId": 1_440_833_542_u64,
                    "collectionId": 1_440_833_098_u64,
                    "trackName": "First Song",
                    "artistName": "Example Artist",
                    "collectionName": "Example Album",
                    "trackCount": 2
                },
                {
                    "wrapperType": "track",
                    "kind": "music-video",
                    "trackId": 1_440_833_999_u64,
                    "collectionId": 1_440_833_098_u64,
                    "trackName": "Not Audio",
                    "artistName": "Example Artist",
                    "trackCount": 2
                }
            ]
        }),
    )]);
    let client = AppleMusicPublicCatalog::with_transport(transport.clone());
    let source = reference("https://music.apple.com/jp/album/example/1440833098");

    let collection = client.resolve(&source, 3).await.expect("album metadata");

    assert_eq!(collection.title, "Example Album");
    assert_eq!(collection.tracks.len(), 1);
    assert_eq!(collection.skipped_item_count, 1);
    assert_eq!(collection.source_item_count, Some(2));
    assert!(!collection.truncated);
    assert_eq!(
        collection.tracks[0].reference.canonical_url().as_str(),
        "https://music.apple.com/jp/album/example/1440833098?i=1440833542"
    );
    assert_eq!(
        transport.request_urls(),
        ["https://itunes.apple.com/lookup?id=1440833098&country=JP&entity=song&limit=4"]
    );
}

#[tokio::test]
async fn album_limit_requests_one_lookahead_track() {
    let transport = ScriptedTransport::new([json(
        200,
        &serde_json::json!({
            "resultCount": 3,
            "results": [
                {
                    "wrapperType": "collection",
                    "collectionType": "Album",
                    "collectionId": 1_440_833_098_u64,
                    "collectionName": "Large Album"
                },
                {
                    "wrapperType": "track",
                    "kind": "song",
                    "trackId": 1_440_833_542_u64,
                    "collectionId": 1_440_833_098_u64,
                    "trackName": "First Song",
                    "artistName": "Example Artist",
                    "trackCount": 3
                },
                {
                    "wrapperType": "track",
                    "kind": "song",
                    "trackId": 1_440_833_543_u64,
                    "collectionId": 1_440_833_098_u64,
                    "trackName": "Second Song",
                    "artistName": "Example Artist",
                    "trackCount": 3
                }
            ]
        }),
    )]);
    let client = AppleMusicPublicCatalog::with_transport(transport.clone());
    let source = reference("https://music.apple.com/jp/album/example/1440833098");

    let collection = client.resolve(&source, 1).await.expect("album metadata");

    assert_eq!(collection.tracks.len(), 1);
    assert_eq!(collection.source_item_count, Some(3));
    assert!(collection.truncated);
    assert_eq!(
        transport.request_urls(),
        ["https://itunes.apple.com/lookup?id=1440833098&country=JP&entity=song&limit=2"]
    );
}

#[tokio::test]
async fn playlist_requires_developer_credentials_without_network_access() {
    let transport = ScriptedTransport::new(Vec::<HttpResponse>::new());
    let client = AppleMusicPublicCatalog::with_transport(transport.clone());
    let source = reference("https://music.apple.com/jp/playlist/example/pl.1234");

    assert_eq!(
        client.resolve(&source, 25).await,
        Err(CatalogError::AppleMusicPlaylistRequiresCredentials)
    );
    assert!(transport.request_urls().is_empty());
}

#[tokio::test]
async fn empty_lookup_is_not_found_and_malformed_counts_fail_closed() {
    let transport = ScriptedTransport::new([
        json(200, &serde_json::json!({"resultCount": 0, "results": []})),
        json(200, &serde_json::json!({"resultCount": 2, "results": []})),
    ]);
    let client = AppleMusicPublicCatalog::with_transport(transport);
    let source = reference("https://music.apple.com/us/song/example/1440833542");

    assert_eq!(
        client.resolve(&source, 25).await,
        Err(CatalogError::NotFound(CatalogProvider::AppleMusic))
    );
    assert_eq!(
        client.resolve(&source, 25).await,
        Err(CatalogError::InvalidResponse(CatalogProvider::AppleMusic))
    );
}

#[tokio::test]
async fn redirect_response_is_not_followed() {
    let transport = ScriptedTransport::new([HttpResponse::new(302, Vec::new())]);
    let client = AppleMusicPublicCatalog::with_transport(transport.clone());
    let source = reference("https://music.apple.com/us/song/example/1440833542");

    assert_eq!(
        client.resolve(&source, 25).await,
        Err(CatalogError::Transport(CatalogProvider::AppleMusic))
    );
    assert_eq!(transport.request_urls().len(), 1);
}
