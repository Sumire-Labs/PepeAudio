use std::path::{Path, PathBuf};

use async_trait::async_trait;
use pepeaudio_media::{DownloadedMedia, InspectedMedia};
use pepeaudio_player::QueueTrack;

use crate::{PipelineError, PipelineResult};

/// Trusted local object passed to a decoder factory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedSource {
    path: PathBuf,
}

impl ResolvedSource {
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    #[must_use]
    pub fn from_downloaded(media: &DownloadedMedia) -> Self {
        Self::new(media.path.clone())
    }

    #[must_use]
    pub fn from_inspected(media: &InspectedMedia) -> Self {
        Self::from_downloaded(&media.download)
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Resolves an opaque queue locator into a trusted local media object.
#[async_trait]
pub trait TrackResolver: Send + Sync {
    /// Resolves one queue track without downloading on the realtime path.
    async fn resolve(&self, track: &QueueTrack) -> PipelineResult<ResolvedSource>;
}

/// Resolver restricted to one canonical media cache root.
#[derive(Clone, Debug)]
pub struct ManagedMediaResolver {
    root: PathBuf,
}

impl ManagedMediaResolver {
    /// Canonicalizes the trusted operator-owned cache root.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::ManagedRoot`] when the root does not exist or
    /// cannot be canonicalized.
    pub async fn new(root: impl AsRef<Path>) -> PipelineResult<Self> {
        let root = tokio::fs::canonicalize(root)
            .await
            .map_err(PipelineError::ManagedRoot)?;
        let metadata = tokio::fs::metadata(&root)
            .await
            .map_err(PipelineError::ManagedRoot)?;
        if !metadata.is_dir() {
            return Err(PipelineError::InvalidSource);
        }
        Ok(Self { root })
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }
}

#[async_trait]
impl TrackResolver for ManagedMediaResolver {
    async fn resolve(&self, track: &QueueTrack) -> PipelineResult<ResolvedSource> {
        let supplied = PathBuf::from(track.source.as_str());
        let candidate = if supplied.is_absolute() {
            supplied
        } else {
            self.root.join(supplied)
        };
        let canonical = tokio::fs::canonicalize(candidate)
            .await
            .map_err(PipelineError::Resolve)?;
        if !canonical.starts_with(&self.root) {
            return Err(PipelineError::InvalidSource);
        }
        let metadata = tokio::fs::metadata(&canonical)
            .await
            .map_err(PipelineError::Resolve)?;
        if !metadata.is_file() {
            return Err(PipelineError::InvalidSource);
        }
        Ok(ResolvedSource::new(canonical))
    }
}

#[cfg(test)]
mod tests;
