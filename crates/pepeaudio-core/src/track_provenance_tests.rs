use super::*;

#[test]
fn accepts_canonical_provider_pages() {
    for (provider, url) in [
        (
            MediaProvider::Spotify,
            "https://open.spotify.com/track/4uLU6hMCjMI75M1A2tKUQC",
        ),
        (
            MediaProvider::AppleMusic,
            "https://music.apple.com/jp/album/example/1440833098?i=1440833542",
        ),
        (
            MediaProvider::YouTube,
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
        ),
        (
            MediaProvider::SoundCloud,
            "https://soundcloud.com/example/example-track",
        ),
    ] {
        assert_eq!(
            PublicMediaPage::new(provider, url)
                .expect("canonical page")
                .provider(),
            provider
        );
    }
}

#[test]
fn rejects_stream_and_token_bearing_urls() {
    for (provider, url) in [
        (
            MediaProvider::YouTube,
            "https://rr1---sn.example.googlevideo.com/videoplayback?sig=secret",
        ),
        (
            MediaProvider::SoundCloud,
            "https://soundcloud.com/example/private-track?secret_token=s-secret",
        ),
        (
            MediaProvider::Spotify,
            "https://cdn.example.test/track/4uLU6hMCjMI75M1A2tKUQC",
        ),
        (
            MediaProvider::YouTube,
            "https://www.youtube.com:444/watch?v=dQw4w9WgXcQ",
        ),
        (
            MediaProvider::YouTube,
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ\n",
        ),
        (
            MediaProvider::AppleMusic,
            "https://music.apple.com/jp/album/extra/example/1440833098?i=1440833542",
        ),
        (
            MediaProvider::AppleMusic,
            "https://music.apple.com/jp//album/example/1440833098?i=1440833542",
        ),
    ] {
        assert!(PublicMediaPage::new(provider, url).is_err(), "{url}");
    }

    let oversized_slug = "a".repeat(MAX_APPLE_SLUG_BYTES + 1);
    let oversized_url = format!("https://music.apple.com/jp/song/{oversized_slug}/1440833542");
    assert!(PublicMediaPage::new(MediaProvider::AppleMusic, oversized_url).is_err());
}

#[test]
fn deserialization_revalidates_public_pages_and_playback_provider() {
    let invalid_page = serde_json::json!({
        "provider": "youtube",
        "url": "https://example.test/watch?v=dQw4w9WgXcQ"
    });
    assert!(serde_json::from_value::<PublicMediaPage>(invalid_page).is_err());

    let non_default_port = serde_json::json!({
        "provider": "youtube",
        "url": "https://www.youtube.com:444/watch?v=dQw4w9WgXcQ"
    });
    assert!(serde_json::from_value::<PublicMediaPage>(non_default_port).is_err());

    let invalid_playback = serde_json::json!({
        "origin": null,
        "playback": {
            "provider": "spotify",
            "url": "https://open.spotify.com/track/4uLU6hMCjMI75M1A2tKUQC"
        }
    });
    assert!(serde_json::from_value::<TrackProvenance>(invalid_playback).is_err());
}
