use pepeaudio_catalog::{CatalogResolverBuilder, SpotifyPublicCatalog};
use url::Url;

#[tokio::test]
#[ignore = "uses Spotify's live public playlist page and public network access"]
async fn resolves_a_live_public_spotify_playlist_without_credentials() {
    let resolver = CatalogResolverBuilder::new()
        .collection_limit(3)
        .expect("collection limit")
        .spotify_public(SpotifyPublicCatalog::new().expect("public Spotify client"))
        .build();
    let url = Url::parse("https://open.spotify.com/playlist/37i9dQZF1DXcBWIGoYBM5M")
        .expect("public Spotify playlist URL");

    let collection = resolver.resolve_url(&url).await.expect("public playlist");

    assert!(!collection.title.is_empty());
    assert!(!collection.tracks.is_empty());
    assert!(collection.tracks.len() <= 3);
    assert!(collection.tracks.iter().all(|track| {
        track.reference.canonical_url().scheme() == "https"
            && track.reference.canonical_url().host_str() == Some("open.spotify.com")
            && !track.title.is_empty()
            && !track.artists.is_empty()
    }));
}
