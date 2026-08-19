use url::Url;

use crate::{CatalogError, CatalogItemKind, CatalogProvider, CatalogReference, CatalogResult};

const SPOTIFY_HOST: &str = "open.spotify.com";
const APPLE_MUSIC_HOST: &str = "music.apple.com";
const MAX_CATALOG_URL_BYTES: usize = 4_096;
const MAX_APPLE_SLUG_BYTES: usize = 512;

/// Parses a copied catalog link without following redirects or accepting
/// look-alike hosts.
///
/// # Errors
///
/// Returns [`CatalogError::UnsupportedUrl`] for non-HTTPS URLs, unknown hosts,
/// malformed identifiers, or unsupported query parameters.
pub fn parse_catalog_url(url: &Url) -> CatalogResult<CatalogReference> {
    validate_origin(url)?;
    match url.host_str() {
        Some(SPOTIFY_HOST) => parse_spotify(url),
        Some(APPLE_MUSIC_HOST) => parse_apple_music(url),
        _ => Err(CatalogError::UnsupportedUrl),
    }
}

fn validate_origin(url: &Url) -> CatalogResult<()> {
    if url.as_str().len() > MAX_CATALOG_URL_BYTES
        || url.scheme() != "https"
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(CatalogError::UnsupportedUrl);
    }
    Ok(())
}

fn parse_spotify(url: &Url) -> CatalogResult<CatalogReference> {
    validate_spotify_query(url)?;
    let mut segments = path_segments(url)?;
    if segments
        .first()
        .is_some_and(|value| value.strip_prefix("intl-").is_some_and(valid_locale_suffix))
    {
        segments.remove(0);
    }
    if segments.len() != 2 || !valid_spotify_id(segments[1]) {
        return Err(CatalogError::UnsupportedUrl);
    }
    let kind = match segments[0] {
        "track" => CatalogItemKind::Track,
        "album" => CatalogItemKind::Album,
        "playlist" => CatalogItemKind::Playlist,
        _ => return Err(CatalogError::UnsupportedUrl),
    };
    let id = segments[1].to_owned();
    let canonical_url = Url::parse(&format!("https://{SPOTIFY_HOST}/{}/{id}", segments[0]))
        .expect("constant Spotify URL is valid");
    Ok(CatalogReference::new(
        CatalogProvider::Spotify,
        kind,
        id,
        None,
        canonical_url,
    ))
}

fn validate_spotify_query(url: &Url) -> CatalogResult<()> {
    let mut saw_share_id = false;
    for (name, value) in url.query_pairs() {
        if name != "si"
            || saw_share_id
            || value.is_empty()
            || value.len() > 128
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        {
            return Err(CatalogError::UnsupportedUrl);
        }
        saw_share_id = true;
    }
    Ok(())
}

fn parse_apple_music(url: &Url) -> CatalogResult<CatalogReference> {
    let segments = path_segments(url)?;
    if segments.len() != 4 || !valid_storefront(segments[0]) || !valid_apple_slug(segments[2]) {
        return Err(CatalogError::UnsupportedUrl);
    }
    let storefront = segments[0].to_owned();
    let path_kind = segments[1];
    let path_id = segments[3];
    let query_song_id = apple_query_song_id(url)?;
    let (kind, id) = match path_kind {
        "album" if valid_apple_numeric_id(path_id) && query_song_id.is_some() => (
            CatalogItemKind::Track,
            query_song_id.expect("checked above"),
        ),
        "album" if valid_apple_numeric_id(path_id) => (CatalogItemKind::Album, path_id.to_owned()),
        "song" if query_song_id.is_none() && valid_apple_numeric_id(path_id) => {
            (CatalogItemKind::Track, path_id.to_owned())
        }
        "playlist" if query_song_id.is_none() && valid_apple_playlist_id(path_id) => {
            (CatalogItemKind::Playlist, path_id.to_owned())
        }
        _ => return Err(CatalogError::UnsupportedUrl),
    };
    let mut canonical_url = Url::parse(&format!("https://{APPLE_MUSIC_HOST}{}", url.path()))
        .expect("validated Apple Music path and constant origin");
    if kind == CatalogItemKind::Track && path_kind == "album" {
        canonical_url.query_pairs_mut().append_pair("i", &id);
    }
    Ok(CatalogReference::new(
        CatalogProvider::AppleMusic,
        kind,
        id,
        Some(storefront),
        canonical_url,
    ))
}

fn apple_query_song_id(url: &Url) -> CatalogResult<Option<String>> {
    let mut song_id = None;
    for (name, value) in url.query_pairs() {
        match name.as_ref() {
            "i" if song_id.is_none() && valid_apple_numeric_id(&value) => {
                song_id = Some(value.into_owned());
            }
            "l" | "ls" | "at" | "ct" | "itsct" | "itscg" | "app"
                if !value.is_empty() && value.len() <= 256 => {}
            _ => return Err(CatalogError::UnsupportedUrl),
        }
    }
    Ok(song_id)
}

fn path_segments(url: &Url) -> CatalogResult<Vec<&str>> {
    let mut segments = url
        .path_segments()
        .ok_or(CatalogError::UnsupportedUrl)?
        .collect::<Vec<_>>();
    if segments.last() == Some(&"") {
        segments.pop();
    }
    if segments.is_empty() || segments.iter().any(|value| value.is_empty()) {
        Err(CatalogError::UnsupportedUrl)
    } else {
        Ok(segments)
    }
}

fn valid_spotify_id(value: &str) -> bool {
    value.len() == 22 && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

fn valid_storefront(value: &str) -> bool {
    value.len() == 2 && value.bytes().all(|byte| byte.is_ascii_lowercase())
}

fn valid_apple_slug(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_APPLE_SLUG_BYTES
        && value != "."
        && value != ".."
        && value.bytes().all(|byte| byte.is_ascii_graphic())
}

fn valid_locale_suffix(value: &str) -> bool {
    (2..=16).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

fn valid_apple_numeric_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= 20 && value.bytes().all(|byte| byte.is_ascii_digit())
}

fn valid_apple_playlist_id(value: &str) -> bool {
    value.starts_with("pl.")
        && (4..=128).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(value: &str) -> CatalogResult<CatalogReference> {
        parse_catalog_url(&Url::parse(value).expect("test URL"))
    }

    #[test]
    fn parses_spotify_share_links_and_removes_tracking_query() {
        let reference =
            parse("https://open.spotify.com/intl-ja/track/4uLU6hMCjMI75M1A2tKUQC?si=abc_123")
                .expect("supported URL");

        assert_eq!(reference.provider(), CatalogProvider::Spotify);
        assert_eq!(reference.kind(), CatalogItemKind::Track);
        assert_eq!(reference.id(), "4uLU6hMCjMI75M1A2tKUQC");
        assert_eq!(
            reference.canonical_url().as_str(),
            "https://open.spotify.com/track/4uLU6hMCjMI75M1A2tKUQC"
        );
    }

    #[test]
    fn parses_apple_album_song_and_playlist_links() {
        let song = parse("https://music.apple.com/jp/album/example/1440833098?i=1440833542")
            .expect("song URL");
        let playlist = parse("https://music.apple.com/us/playlist/example/pl.1234_ab-CD")
            .expect("playlist URL");

        assert_eq!(song.kind(), CatalogItemKind::Track);
        assert_eq!(song.id(), "1440833542");
        assert_eq!(song.storefront(), Some("jp"));
        assert_eq!(
            song.canonical_url().as_str(),
            "https://music.apple.com/jp/album/example/1440833098?i=1440833542"
        );
        assert_eq!(playlist.kind(), CatalogItemKind::Playlist);
        assert_eq!(playlist.id(), "pl.1234_ab-CD");
    }

    #[test]
    fn rejects_lookalikes_credentials_ports_and_unknown_queries() {
        for value in [
            "https://open.spotify.com.evil.test/track/4uLU6hMCjMI75M1A2tKUQC",
            "http://open.spotify.com/track/4uLU6hMCjMI75M1A2tKUQC",
            "https://user@open.spotify.com/track/4uLU6hMCjMI75M1A2tKUQC",
            "https://open.spotify.com:444/track/4uLU6hMCjMI75M1A2tKUQC",
            "https://open.spotify.com/track/4uLU6hMCjMI75M1A2tKUQC?redirect=x",
            "https://open.spotify.com/track/4uLU6hMCjMI75M1A2tKUQC?si=first&si=second",
            "https://music.apple.com/jp/album/example/1440833098?i=not-a-song",
            "https://music.apple.com/jp/album/extra/example/1440833098",
            "https://music.apple.com/jp/album/example/not-an-album?i=1440833542",
            "https://music.apple.com/jp/song/example/1440833542?i=1440833542",
            "https://music.apple.com/jp//album/example/1440833098",
        ] {
            assert_eq!(parse(value), Err(CatalogError::UnsupportedUrl), "{value}");
        }
    }

    #[test]
    fn rejects_oversized_catalog_urls_and_apple_slugs() {
        let oversized_slug = "a".repeat(MAX_APPLE_SLUG_BYTES + 1);
        let oversized_slug_url =
            format!("https://music.apple.com/jp/album/{oversized_slug}/1440833098");
        assert_eq!(
            parse(&oversized_slug_url),
            Err(CatalogError::UnsupportedUrl)
        );

        let oversized_url = format!(
            "https://open.spotify.com/track/4uLU6hMCjMI75M1A2tKUQC?si={}",
            "a".repeat(MAX_CATALOG_URL_BYTES)
        );
        assert_eq!(parse(&oversized_url), Err(CatalogError::UnsupportedUrl));
    }
}
