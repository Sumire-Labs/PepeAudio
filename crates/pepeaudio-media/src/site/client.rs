use std::{sync::Arc, time::Duration};

use crate::{OutputLimits, ProcessRunner};

use super::{
    SiteCollection, SiteError, SiteProvider, SiteReference, SiteResolvedTrack, SiteSearch,
    YtDlpConfig, command, matching, parse,
};

const DISCOVERY_STDOUT_BYTES: usize = 4 * 1024 * 1024;
const RESOLVE_STDOUT_BYTES: usize = 2 * 1024 * 1024;
const STDERR_BYTES: usize = 64 * 1024;
const VERSION_OUTPUT_BYTES: usize = 4 * 1024;
const MINIMUM_YTDLP_VERSION: (u32, u32, u32) = (2026, 6, 9);
const MINIMUM_DENO_VERSION: (u32, u32, u32) = (2, 3, 0);

#[derive(Clone)]
pub struct YtDlpClient {
    config: YtDlpConfig,
    runner: Arc<dyn ProcessRunner>,
}

impl YtDlpClient {
    /// Creates a client around an explicitly configured process runner.
    ///
    /// # Errors
    ///
    /// Returns [`SiteError::InvalidConfig`] for invalid executable, cache, or
    /// playlist bounds.
    pub fn new(config: YtDlpConfig, runner: Arc<dyn ProcessRunner>) -> Result<Self, SiteError> {
        config.validate()?;
        Ok(Self { config, runner })
    }

    /// Discovers one supported page or a bounded prefix of a playlist.
    ///
    /// # Errors
    ///
    /// Returns a URL, process, metadata, or configured-limit error.
    pub async fn discover_url(
        &self,
        raw_url: &str,
        maximum_items: usize,
    ) -> Result<SiteCollection, SiteError> {
        if maximum_items == 0 || maximum_items > self.config.maximum_playlist_items {
            return Err(SiteError::InvalidConfig);
        }
        let provider = SiteProvider::classify(raw_url)?.ok_or(SiteError::InvalidUrl)?;
        let specification = command::discover(
            &self.config,
            provider,
            raw_url,
            maximum_items.saturating_add(1),
        );
        let output = self
            .runner
            .run(
                &specification,
                OutputLimits {
                    timeout: Duration::from_mins(1),
                    max_stdout_bytes: DISCOVERY_STDOUT_BYTES,
                    max_stderr_bytes: STDERR_BYTES,
                },
            )
            .await?;
        parse::collection(&output.stdout, provider, raw_url, maximum_items)
    }

    /// Verifies both explicitly configured executables before Discord starts.
    ///
    /// # Errors
    ///
    /// Returns a process or unsupported-tool-version error.
    pub async fn verify_tools(&self) -> Result<(), SiteError> {
        let limits = OutputLimits {
            timeout: Duration::from_secs(5),
            max_stdout_bytes: VERSION_OUTPUT_BYTES,
            max_stderr_bytes: VERSION_OUTPUT_BYTES,
        };
        let ytdlp = self
            .runner
            .run(&command::ytdlp_version(&self.config), limits)
            .await?;
        verify_ytdlp_version(&ytdlp.stdout)?;
        let deno = self
            .runner
            .run(&command::deno_version(&self.config), limits)
            .await?;
        verify_deno_version(&deno.stdout)
    }

    /// Resolves one discovered page to a safe direct-audio request.
    ///
    /// # Errors
    ///
    /// Returns a process, metadata, duration, header, or stream-policy error.
    pub async fn resolve(&self, reference: &SiteReference) -> Result<SiteResolvedTrack, SiteError> {
        let output = self
            .runner
            .run(
                &command::resolve(&self.config, reference),
                OutputLimits {
                    timeout: Duration::from_secs(45),
                    max_stdout_bytes: RESOLVE_STDOUT_BYTES,
                    max_stderr_bytes: STDERR_BYTES,
                },
            )
            .await?;
        parse::resolved(&output.stdout, reference, &self.config)
    }

    /// Finds a high-confidence `YouTube` result, then tries `SoundCloud`.
    ///
    /// # Errors
    ///
    /// Returns [`SiteError::NoSearchMatch`] when no unambiguous candidate is
    /// safe. Operational and security errors are propagated without fallback.
    pub async fn resolve_search(
        &self,
        search: &SiteSearch,
    ) -> Result<SiteResolvedTrack, SiteError> {
        for provider in [SiteProvider::YouTube, SiteProvider::SoundCloud] {
            match self.search_provider(search, provider).await {
                Ok(track) => return Ok(track),
                Err(SiteError::NoSearchMatch | SiteError::UnsupportedStream) => {}
                Err(error) => return Err(error),
            }
        }
        Err(SiteError::NoSearchMatch)
    }

    async fn search_provider(
        &self,
        search: &SiteSearch,
        provider: SiteProvider,
    ) -> Result<SiteResolvedTrack, SiteError> {
        let output = self
            .runner
            .run(
                &command::search(&self.config, provider, &search.query),
                OutputLimits {
                    timeout: Duration::from_secs(45),
                    max_stdout_bytes: RESOLVE_STDOUT_BYTES,
                    max_stderr_bytes: STDERR_BYTES,
                },
            )
            .await?;
        let input = format!("{}{}", provider.search_prefix(), search.query);
        let collection = parse::search_collection(&output.stdout, provider, &input, 5)?;
        let candidates = matching::ranked_candidates(&collection.entries, search)?;
        for reference in candidates {
            match self.resolve(reference).await {
                Ok(resolved)
                    if matching::duration_matches(
                        search.preferred_duration_ms,
                        resolved.duration_ms,
                    ) =>
                {
                    return Ok(resolved);
                }
                Ok(_) | Err(SiteError::NoSearchMatch | SiteError::UnsupportedStream) => {}
                Err(error) => return Err(error),
            }
        }
        Err(SiteError::NoSearchMatch)
    }
}

fn verify_ytdlp_version(bytes: &[u8]) -> Result<(), SiteError> {
    let value = std::str::from_utf8(bytes).map_err(|_| SiteError::UnsupportedToolVersion)?;
    let version = value
        .lines()
        .next()
        .and_then(parse_version)
        .ok_or(SiteError::UnsupportedToolVersion)?;
    (version >= MINIMUM_YTDLP_VERSION)
        .then_some(())
        .ok_or(SiteError::UnsupportedToolVersion)
}

fn verify_deno_version(bytes: &[u8]) -> Result<(), SiteError> {
    let value = std::str::from_utf8(bytes).map_err(|_| SiteError::UnsupportedToolVersion)?;
    let version = value
        .lines()
        .next()
        .and_then(|line| line.strip_prefix("deno "))
        .and_then(|line| line.split_whitespace().next())
        .and_then(parse_version)
        .ok_or(SiteError::UnsupportedToolVersion)?;
    (version >= MINIMUM_DENO_VERSION)
        .then_some(())
        .ok_or(SiteError::UnsupportedToolVersion)
}

fn parse_version(value: &str) -> Option<(u32, u32, u32)> {
    let mut parts = value.trim().split('.');
    let version = (
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
    );
    parts.next().is_none().then_some(version)
}
