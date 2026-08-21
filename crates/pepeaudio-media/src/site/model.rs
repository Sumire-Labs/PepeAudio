use std::{fmt, path::PathBuf, time::Duration};

use crate::{MediaRequest, ProcessError};

pub(super) const HARD_MAX_PLAYLIST_ITEMS: usize = 100;
const MAX_PAGE_URL_BYTES: usize = 4_096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SiteProvider {
    YouTube,
    SoundCloud,
}

impl SiteProvider {
    /// Classifies only supported page hosts. Other HTTPS URLs remain eligible
    /// for the direct-media path.
    ///
    /// # Errors
    ///
    /// Returns [`SiteError::InvalidUrl`] for oversized, malformed, or unsafe
    /// page URLs.
    pub fn classify(raw: &str) -> Result<Option<Self>, SiteError> {
        if raw.len() > MAX_PAGE_URL_BYTES || raw.chars().any(char::is_control) {
            return Err(SiteError::InvalidUrl);
        }
        let url = url::Url::parse(raw).map_err(|_| SiteError::InvalidUrl)?;
        if url.scheme() != "https"
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
            || url.port_or_known_default() != Some(443)
        {
            return Err(SiteError::InvalidUrl);
        }
        let host = url.host_str().ok_or(SiteError::InvalidUrl)?;
        Ok([Self::YouTube, Self::SoundCloud]
            .into_iter()
            .find(|provider| provider.accepts_page_host(host)))
    }

    pub(crate) fn accepts_page_host(self, host: &str) -> bool {
        let host = host.trim_end_matches('.');
        match self {
            Self::YouTube => matches!(
                host,
                "youtube.com"
                    | "www.youtube.com"
                    | "m.youtube.com"
                    | "music.youtube.com"
                    | "youtu.be"
            ),
            Self::SoundCloud => matches!(
                host,
                "soundcloud.com" | "www.soundcloud.com" | "m.soundcloud.com" | "on.soundcloud.com"
            ),
        }
    }

    pub(crate) fn accepts_media_host(self, host: &str) -> bool {
        let host = host.trim_end_matches('.');
        match self {
            Self::YouTube => host == "googlevideo.com" || host.ends_with(".googlevideo.com"),
            Self::SoundCloud => host == "sndcdn.com" || host.ends_with(".sndcdn.com"),
        }
    }

    pub(crate) const fn extractor_allowlist(self) -> &'static str {
        match self {
            Self::YouTube => "youtube.*,end",
            Self::SoundCloud => "soundcloud.*,end",
        }
    }

    pub(crate) const fn search_prefix(self) -> &'static str {
        match self {
            Self::YouTube => "ytsearch5:",
            Self::SoundCloud => "scsearch5:",
        }
    }

    pub(crate) const fn single_search_prefix(self) -> &'static str {
        match self {
            Self::YouTube => "ytsearch1:",
            Self::SoundCloud => "scsearch1:",
        }
    }

    pub(crate) const fn format_selector(self) -> &'static str {
        match self {
            Self::YouTube => "bestaudio[protocol=https][vcodec=none][acodec!=none]",
            Self::SoundCloud => "bestaudio[protocol=http][vcodec=none][acodec!=none]",
        }
    }

    pub(crate) fn is_single_item_url(self, raw: &str) -> Result<bool, SiteError> {
        let url = url::Url::parse(raw).map_err(|_| SiteError::InvalidUrl)?;
        if url.query_pairs().any(|(name, _)| name == "list") {
            return Ok(false);
        }
        let segments = url
            .path_segments()
            .map(|segments| segments.filter(|part| !part.is_empty()).collect::<Vec<_>>())
            .ok_or(SiteError::InvalidUrl)?;
        Ok(match self {
            Self::YouTube => {
                (url.host_str() == Some("youtu.be") && segments.len() == 1)
                    || (segments.as_slice() == ["watch"]
                        && url.query_pairs().any(|(name, value)| {
                            name == "v"
                                && value.len() == 11
                                && value.bytes().all(|byte| {
                                    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')
                                })
                        }))
            }
            Self::SoundCloud => url.host_str() == Some("on.soundcloud.com") || segments.len() == 2,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct YtDlpConfig {
    pub executable: PathBuf,
    pub deno_executable: PathBuf,
    pub deno_directory: PathBuf,
    pub maximum_track_duration: Duration,
    pub maximum_playlist_items: usize,
}

impl YtDlpConfig {
    pub(crate) fn validate(&self) -> Result<(), SiteError> {
        if self.executable.as_os_str().is_empty()
            || self.deno_executable.as_os_str().is_empty()
            || self.deno_directory.as_os_str().is_empty()
            || self.maximum_track_duration.is_zero()
            || self.maximum_playlist_items == 0
            || self.maximum_playlist_items > HARD_MAX_PLAYLIST_ITEMS
        {
            return Err(SiteError::InvalidConfig);
        }
        Ok(())
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct SiteReference {
    pub provider: SiteProvider,
    pub(crate) page_url: String,
    pub(crate) title: Option<String>,
    pub(crate) artist: Option<String>,
    pub(crate) duration_ms: Option<u64>,
}

impl SiteReference {
    #[must_use]
    pub fn page_url(&self) -> &str {
        &self.page_url
    }
}

impl fmt::Debug for SiteReference {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SiteReference")
            .field("provider", &self.provider)
            .field("page_url", &"<redacted>")
            .field("has_title", &self.title.is_some())
            .field("has_artist", &self.artist.is_some())
            .field("duration_ms", &self.duration_ms)
            .finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SiteCollection {
    pub title: Option<String>,
    pub entries: Vec<SiteReference>,
    pub source_item_count: Option<usize>,
    pub skipped_items: usize,
    pub truncated: bool,
}

#[derive(Debug)]
pub struct SiteResolvedTrack {
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub provider: SiteProvider,
    /// Canonical public provider page. This is never a CDN or signed stream URL.
    pub page_url: String,
    pub duration_ms: u64,
    pub request: MediaRequest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SiteSearch {
    pub query: String,
    pub expected_title: String,
    pub expected_artists: Vec<String>,
    pub preferred_duration_ms: Option<u64>,
    pub isrc: Option<String>,
}

impl SiteSearch {
    /// Creates a bounded structured search request.
    ///
    /// # Errors
    ///
    /// Returns [`SiteError::InvalidSearch`] for empty, oversized, controlled,
    /// or malformed metadata.
    pub fn new(
        query: impl Into<String>,
        expected_title: impl Into<String>,
        expected_artists: Vec<String>,
        preferred_duration_ms: Option<u64>,
        isrc: Option<String>,
    ) -> Result<Self, SiteError> {
        let query = query.into();
        let expected_title = expected_title.into();
        if !valid_text(&query, 256)
            || !valid_text(&expected_title, 200)
            || expected_artists.len() > 16
            || expected_artists
                .iter()
                .any(|artist| !valid_text(artist, 200))
            || isrc.as_ref().is_some_and(|value| !valid_isrc(value))
        {
            return Err(SiteError::InvalidSearch);
        }
        Ok(Self {
            query,
            expected_title,
            expected_artists,
            preferred_duration_ms,
            isrc,
        })
    }
}

fn valid_text(value: &str, maximum: usize) -> bool {
    !value.trim().is_empty() && value.len() <= maximum && !value.chars().any(char::is_control)
}

fn valid_isrc(value: &str) -> bool {
    value.len() == 12 && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

#[derive(Debug, thiserror::Error)]
pub enum SiteError {
    #[error("site extractor configuration is invalid")]
    InvalidConfig,
    #[error("site extractor tools are unavailable or below the supported version")]
    UnsupportedToolVersion,
    #[error("URL is not a supported YouTube or SoundCloud page")]
    InvalidUrl,
    #[error("playlist exceeds the configured {maximum}-item limit")]
    PlaylistTooLarge { maximum: usize },
    #[error("site extractor returned invalid metadata")]
    InvalidMetadata,
    #[error("site extractor returned a live, manifest, or unsafe media stream")]
    UnsupportedStream,
    #[error("site media exceeds the configured duration limit")]
    DurationLimit,
    #[error("site extractor returned an unsafe request header")]
    UnsafeHeader,
    #[error("site search query is invalid")]
    InvalidSearch,
    #[error("no safe cross-service search match was found")]
    NoSearchMatch,
    #[error(transparent)]
    Process(ProcessError),
}

impl From<ProcessError> for SiteError {
    fn from(error: ProcessError) -> Self {
        match error {
            ProcessError::MediaUnavailable => Self::UnsupportedStream,
            error => Self::Process(error),
        }
    }
}
