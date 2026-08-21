use std::{fmt, path::PathBuf, time::Duration};

use crate::{SafeHttpHeaders, SiteProvider};

#[derive(Clone, Eq, PartialEq)]
pub enum MediaRequest {
    DirectUrl { url: String },
    DiscordAttachment(DiscordAttachment),
    ResolvedSite(ResolvedSiteMedia),
}

impl MediaRequest {
    /// Returns the untrusted URL to validate and fetch.
    #[must_use]
    pub fn url(&self) -> &str {
        match self {
            Self::DirectUrl { url } => url,
            Self::DiscordAttachment(attachment) => &attachment.url,
            Self::ResolvedSite(site) => &site.url,
        }
    }

    #[must_use]
    pub const fn source_kind(&self) -> MediaSourceKind {
        match self {
            Self::DirectUrl { .. } => MediaSourceKind::DirectUrl,
            Self::DiscordAttachment(_) => MediaSourceKind::DiscordAttachment,
            Self::ResolvedSite(_) => MediaSourceKind::ResolvedSite,
        }
    }

    pub(crate) const fn declared_size(&self) -> Option<u64> {
        match self {
            Self::DiscordAttachment(attachment) => attachment.declared_size_bytes,
            Self::DirectUrl { .. } | Self::ResolvedSite(_) => None,
        }
    }

    pub(crate) fn headers(&self) -> &SafeHttpHeaders {
        match self {
            Self::ResolvedSite(site) => &site.headers,
            Self::DirectUrl { .. } | Self::DiscordAttachment(_) => empty_headers(),
        }
    }

    pub(crate) const fn uses_open_range(&self) -> bool {
        matches!(
            self,
            Self::ResolvedSite(ResolvedSiteMedia {
                provider: SiteProvider::YouTube,
                ..
            })
        )
    }

    pub(crate) fn allows_host(&self, host: &str) -> bool {
        match self {
            Self::ResolvedSite(site) => site.provider.accepts_media_host(host),
            Self::DirectUrl { .. } | Self::DiscordAttachment(_) => true,
        }
    }
}

impl fmt::Debug for MediaRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MediaRequest")
            .field("source_kind", &self.source_kind())
            .field("url", &"<redacted>")
            .finish()
    }
}

fn empty_headers() -> &'static SafeHttpHeaders {
    static EMPTY: std::sync::OnceLock<SafeHttpHeaders> = std::sync::OnceLock::new();
    EMPTY.get_or_init(SafeHttpHeaders::default)
}

#[derive(Clone, Eq, PartialEq)]
pub struct ResolvedSiteMedia {
    pub(crate) url: String,
    pub(crate) provider: SiteProvider,
    pub(crate) headers: SafeHttpHeaders,
}

impl ResolvedSiteMedia {
    pub(crate) fn new(url: String, provider: SiteProvider, headers: SafeHttpHeaders) -> Self {
        Self {
            url,
            provider,
            headers,
        }
    }
}

impl fmt::Debug for ResolvedSiteMedia {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResolvedSiteMedia")
            .field("url", &"<redacted>")
            .field("provider", &self.provider)
            .field("headers", &self.headers)
            .finish()
    }
}

/// Untrusted Discord attachment metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscordAttachment {
    /// CDN URL. It receives no host-based exemption.
    pub url: String,
    /// Display-only original filename. It never selects a decoder.
    pub filename: String,
    /// Display-only content type supplied by Discord.
    pub content_type: Option<String>,
    /// Size reported by Discord, used only for an early upper-bound check.
    pub declared_size_bytes: Option<u64>,
}

/// Origin category retained with a downloaded object.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MediaSourceKind {
    DirectUrl,
    DiscordAttachment,
    ResolvedSite,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FetchLimits {
    pub max_url_bytes: usize,
    pub max_redirects: usize,
    pub max_download_bytes: u64,
    /// Time allowed to resolve redirects and receive final headers.
    pub redirect_timeout: Duration,
    /// Time allowed to stream the final response to disk.
    pub download_timeout: Duration,
    pub dns_timeout: Duration,
    /// Maximum TCP/TLS connection duration for the production adapter.
    pub connect_timeout: Duration,
}

impl Default for FetchLimits {
    fn default() -> Self {
        Self {
            max_url_bytes: 4_096,
            max_redirects: 5,
            max_download_bytes: 100 * 1024 * 1024,
            redirect_timeout: Duration::from_secs(20),
            download_timeout: Duration::from_mins(2),
            dns_timeout: Duration::from_secs(5),
            connect_timeout: Duration::from_secs(10),
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct DownloadedMedia {
    /// Extensionless generated cache path.
    pub path: PathBuf,
    /// Final URL after all validated redirects.
    pub final_url: String,
    pub size_bytes: u64,
    /// Untrusted response content type, retained only as metadata.
    pub content_type: Option<String>,
    pub source_kind: MediaSourceKind,
}

impl fmt::Debug for DownloadedMedia {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DownloadedMedia")
            .field("path", &"<redacted>")
            .field("final_url", &"<redacted>")
            .field("size_bytes", &self.size_bytes)
            .field("content_type", &self.content_type)
            .field("source_kind", &self.source_kind)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DiscordAttachment, DownloadedMedia, MediaRequest, MediaSourceKind, ResolvedSiteMedia,
        SafeHttpHeaders, SiteProvider,
    };

    fn resolved_site(provider: SiteProvider) -> MediaRequest {
        MediaRequest::ResolvedSite(ResolvedSiteMedia::new(
            "https://media.example/audio".to_owned(),
            provider,
            SafeHttpHeaders::default(),
        ))
    }

    #[test]
    fn open_range_is_limited_to_youtube_resolved_site_requests() {
        assert!(resolved_site(SiteProvider::YouTube).uses_open_range());
        assert!(!resolved_site(SiteProvider::SoundCloud).uses_open_range());
        assert!(
            !MediaRequest::DirectUrl {
                url: "https://media.example/audio".to_owned(),
            }
            .uses_open_range()
        );
        assert!(
            !MediaRequest::DiscordAttachment(DiscordAttachment {
                url: "https://cdn.discord.example/audio".to_owned(),
                filename: "audio".to_owned(),
                content_type: None,
                declared_size_bytes: None,
            })
            .uses_open_range()
        );
    }

    #[test]
    fn downloaded_media_debug_omits_paths_and_signed_urls() {
        let media = DownloadedMedia {
            path: "C:/private/sentinel-object".into(),
            final_url: "https://cdn.example/media?token=sentinel-secret".into(),
            size_bytes: 42,
            content_type: Some("audio/webm".into()),
            source_kind: MediaSourceKind::ResolvedSite,
        };
        let debug = format!("{media:?}");
        assert!(!debug.contains("sentinel"));
        assert!(!debug.contains("token="));
    }
}

/// Container and audio metadata parsed from `ffprobe` JSON.
#[derive(Clone, Debug, PartialEq)]
pub struct ProbeMetadata {
    pub format_name: Option<String>,
    /// Duration in seconds when present and finite.
    pub duration_seconds: Option<f64>,
    pub audio_streams: Vec<ProbeStream>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProbeStream {
    pub index: u32,
    pub codec_name: Option<String>,
    /// Sample rate when reported as a valid integer.
    pub sample_rate_hz: Option<u32>,
    pub channels: Option<u16>,
    pub channel_layout: Option<String>,
}
