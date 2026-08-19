use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
use url::Url;

const MAX_PUBLIC_PAGE_URL_BYTES: usize = 2_048;
const MAX_APPLE_SLUG_BYTES: usize = 512;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MediaProvider {
    #[serde(rename = "spotify")]
    Spotify,
    #[serde(rename = "apple_music")]
    AppleMusic,
    #[serde(rename = "youtube")]
    YouTube,
    #[serde(rename = "soundcloud")]
    SoundCloud,
}

impl MediaProvider {
    const fn is_playback_provider(self) -> bool {
        matches!(self, Self::YouTube | Self::SoundCloud)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PublicMediaPage {
    provider: MediaProvider,
    url: String,
}

impl PublicMediaPage {
    /// Creates a provider page reference after verifying its canonical public
    /// host and shape. CDN, stream, signed, and token-bearing URLs are rejected.
    ///
    /// # Errors
    ///
    /// Returns [`PublicMediaPageError::InvalidUrl`] for malformed or non-public
    /// URLs and [`PublicMediaPageError::ProviderMismatch`] for the wrong host or
    /// path shape.
    pub fn new(
        provider: MediaProvider,
        value: impl AsRef<str>,
    ) -> Result<Self, PublicMediaPageError> {
        let value = value.as_ref();
        if value.is_empty()
            || value.len() > MAX_PUBLIC_PAGE_URL_BYTES
            || value.chars().any(char::is_control)
        {
            return Err(PublicMediaPageError::InvalidUrl);
        }
        let url = Url::parse(value).map_err(|_| PublicMediaPageError::InvalidUrl)?;
        if url.scheme() != "https"
            || url.port().is_some()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
        {
            return Err(PublicMediaPageError::InvalidUrl);
        }
        let valid = match provider {
            MediaProvider::Spotify => valid_spotify(&url),
            MediaProvider::AppleMusic => valid_apple_music(&url),
            MediaProvider::YouTube => valid_youtube(&url),
            MediaProvider::SoundCloud => valid_soundcloud(&url),
        };
        if !valid {
            return Err(PublicMediaPageError::ProviderMismatch);
        }
        Ok(Self {
            provider,
            url: url.to_string(),
        })
    }

    #[must_use]
    pub const fn provider(&self) -> MediaProvider {
        self.provider
    }

    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }
}

impl<'de> Deserialize<'de> for PublicMediaPage {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawPublicMediaPage::deserialize(deserializer)?;
        Self::new(raw.provider, raw.url).map_err(D::Error::custom)
    }
}

#[derive(Deserialize)]
struct RawPublicMediaPage {
    provider: MediaProvider,
    url: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TrackProvenance {
    origin: Option<PublicMediaPage>,
    playback: PublicMediaPage,
}

impl TrackProvenance {
    /// Builds source attribution from stable public pages only.
    ///
    /// # Errors
    ///
    /// Returns [`PublicMediaPageError::InvalidPlaybackProvider`] unless the
    /// playback page belongs to `YouTube` or `SoundCloud`.
    pub fn new(
        origin: Option<PublicMediaPage>,
        playback: PublicMediaPage,
    ) -> Result<Self, PublicMediaPageError> {
        if !playback.provider().is_playback_provider() {
            return Err(PublicMediaPageError::InvalidPlaybackProvider);
        }
        Ok(Self { origin, playback })
    }

    #[must_use]
    pub const fn origin(&self) -> Option<&PublicMediaPage> {
        self.origin.as_ref()
    }

    #[must_use]
    pub const fn playback(&self) -> &PublicMediaPage {
        &self.playback
    }
}

impl<'de> Deserialize<'de> for TrackProvenance {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawTrackProvenance::deserialize(deserializer)?;
        Self::new(raw.origin, raw.playback).map_err(D::Error::custom)
    }
}

#[derive(Deserialize)]
struct RawTrackProvenance {
    #[serde(default)]
    origin: Option<PublicMediaPage>,
    playback: PublicMediaPage,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum PublicMediaPageError {
    #[error("the public media page URL is invalid")]
    InvalidUrl,
    #[error("the public media page does not match its provider")]
    ProviderMismatch,
    #[error("the playback page must belong to YouTube or SoundCloud")]
    InvalidPlaybackProvider,
}

fn valid_spotify(url: &Url) -> bool {
    url.host_str() == Some("open.spotify.com")
        && url.query().is_none()
        && exact_track_path(url, "track", valid_spotify_id)
}

fn valid_apple_music(url: &Url) -> bool {
    if url.host_str() != Some("music.apple.com") {
        return false;
    }
    let Some(segments) = segments(url) else {
        return false;
    };
    if segments.len() != 4
        || segments[0].len() != 2
        || !segments[0].bytes().all(|byte| byte.is_ascii_lowercase())
        || !valid_apple_slug(segments[2])
        || !valid_numeric_id(segments[3])
    {
        return false;
    }
    match segments[1] {
        "song" => url.query().is_none(),
        "album" => one_query_value(url, "i").is_some_and(|value| valid_numeric_id(&value)),
        _ => false,
    }
}

fn valid_youtube(url: &Url) -> bool {
    match url.host_str() {
        Some("youtube.com" | "www.youtube.com") => {
            url.path() == "/watch"
                && one_query_value(url, "v").is_some_and(|value| valid_youtube_id(&value))
        }
        Some("youtu.be") => {
            url.query().is_none() && exact_single_segment(url).is_some_and(valid_youtube_id)
        }
        _ => false,
    }
}

fn valid_soundcloud(url: &Url) -> bool {
    matches!(
        url.host_str(),
        Some("soundcloud.com" | "www.soundcloud.com" | "m.soundcloud.com")
    ) && url.query().is_none()
        && segments(url).is_some_and(|segments| {
            segments.len() == 2
                && segments
                    .iter()
                    .all(|segment| !segment.is_empty() && segment.len() <= 256)
        })
}

fn exact_track_path(url: &Url, kind: &str, id_validator: fn(&str) -> bool) -> bool {
    segments(url).is_some_and(|segments| {
        segments.len() == 2 && segments[0] == kind && id_validator(segments[1])
    })
}

fn exact_single_segment(url: &Url) -> Option<&str> {
    let segments = segments(url)?;
    (segments.len() == 1).then_some(segments[0])
}

fn segments(url: &Url) -> Option<Vec<&str>> {
    let mut segments = url.path_segments()?.collect::<Vec<_>>();
    if segments.last() == Some(&"") {
        segments.pop();
    }
    (!segments.is_empty() && segments.iter().all(|segment| !segment.is_empty())).then_some(segments)
}

fn one_query_value(url: &Url, expected_name: &str) -> Option<String> {
    let mut pairs = url.query_pairs();
    let (name, value) = pairs.next()?;
    if name != expected_name || pairs.next().is_some() {
        return None;
    }
    Some(value.into_owned())
}

fn valid_spotify_id(value: &str) -> bool {
    value.len() == 22 && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

fn valid_numeric_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= 20 && value.bytes().all(|byte| byte.is_ascii_digit())
}

fn valid_apple_slug(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_APPLE_SLUG_BYTES
        && value != "."
        && value != ".."
        && value.bytes().all(|byte| byte.is_ascii_graphic())
}

fn valid_youtube_id(value: &str) -> bool {
    value.len() == 11
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

#[cfg(test)]
#[path = "track_provenance_tests.rs"]
mod tests;
