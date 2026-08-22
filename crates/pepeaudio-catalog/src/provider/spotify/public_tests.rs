use super::*;
use crate::{
    http::{HttpResponse, tests::ScriptedTransport},
    parse_catalog_url,
};

const TRACK_URL: &str = "https://open.spotify.com/track/4uLU6hMCjMI75M1A2tKUQC";
const TRACK_FIXTURE: &str = include_str!("fixtures/public_track.html");

fn reference(value: &str) -> CatalogReference {
    parse_catalog_url(&Url::parse(value).expect("URL")).expect("catalog reference")
}

#[tokio::test]
async fn resolves_bounded_public_track_metadata_without_credentials() {
    assert_eq!(
        meta_value(TRACK_FIXTURE, "og:title", 512).as_deref(),
        Some("Song & Story")
    );
    assert_eq!(
        meta_value(TRACK_FIXTURE, "og:description", 1024).as_deref(),
        Some("Primary & Guest · Example Album · Song · 2026")
    );
    let transport =
        ScriptedTransport::new([HttpResponse::new(200, TRACK_FIXTURE.as_bytes().to_vec())]);
    let client = SpotifyPublicCatalog::with_transport(transport.clone());

    let collection = client
        .resolve(&reference(TRACK_URL), 25)
        .await
        .expect("public metadata");

    assert_eq!(collection.title, "Song & Story");
    assert_eq!(collection.tracks.len(), 1);
    assert_eq!(collection.tracks[0].artists, ["Primary & Guest"]);
    assert_eq!(collection.tracks[0].album, None);
    assert_eq!(collection.tracks[0].duration_ms, Some(213_000));
    assert_eq!(collection.tracks[0].isrc, None);
    assert_eq!(transport.request_urls(), [TRACK_URL]);
    assert_eq!(
        transport.request_response_limit(0),
        Some(MAX_PUBLIC_RESPONSE_BYTES)
    );
    assert_eq!(transport.request_header(0, "authorization"), None);
    assert_eq!(transport.request_header(0, "cookie"), None);
}

#[test]
fn ignores_missing_malformed_or_unbounded_public_duration() {
    assert_eq!(public_duration_ms("359"), Some(359_000));
    assert_eq!(public_duration_ms("0"), None);
    assert_eq!(public_duration_ms("unknown"), None);
    assert_eq!(public_duration_ms("604801"), None);
}

#[tokio::test]
async fn rejects_public_collections_before_network_access() {
    let transport = ScriptedTransport::new(Vec::<HttpResponse>::new());
    let client = SpotifyPublicCatalog::with_transport(transport.clone());

    for (url, kind) in [
        (
            "https://open.spotify.com/album/4aawyAB9vmqN3uQ7FjRGTy",
            CatalogItemKind::Album,
        ),
        (
            "https://open.spotify.com/playlist/37i9dQZF1DXcBWIGoYBM5M",
            CatalogItemKind::Playlist,
        ),
    ] {
        assert_eq!(
            client.resolve(&reference(url), 25).await,
            Err(CatalogError::PublicMetadataUnsupported {
                provider: CatalogProvider::Spotify,
                kind,
            })
        );
    }
    assert!(transport.request_urls().is_empty());
}

#[tokio::test]
async fn malformed_public_metadata_returns_a_redacted_error() {
    let transport = ScriptedTransport::new([HttpResponse::new(
        200,
        br#"<meta property="og:title" content="Only a title">"#.to_vec(),
    )]);
    let client = SpotifyPublicCatalog::with_transport(transport);

    let error = client
        .resolve(&reference(TRACK_URL), 25)
        .await
        .expect_err("missing artist metadata");

    assert_eq!(
        error,
        CatalogError::InvalidResponse(CatalogProvider::Spotify)
    );
    assert_eq!(
        error.to_string(),
        "Spotify returned an unsupported or malformed response"
    );
    assert!(!error.to_string().contains("4uLU6hMCjMI75M1A2tKUQC"));
}
