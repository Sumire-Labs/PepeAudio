use std::path::Path;

use async_trait::async_trait;

use crate::{
    DnsResolver, DownloadedMedia, HttpTransport, IngestError, MediaFetcher, MediaRequest,
    ProbeMetadata, ProcessError,
};

/// Inspection boundary used after a complete bounded download.
#[async_trait]
pub trait MediaProbe: Send + Sync {
    /// Inspects an extensionless local object without making network requests.
    async fn probe(&self, path: &Path) -> Result<ProbeMetadata, ProcessError>;
}

#[derive(Clone, Debug, PartialEq)]
pub struct InspectedMedia {
    pub download: DownloadedMedia,
    /// `ffprobe`-compatible metadata with at least one audio stream.
    pub metadata: ProbeMetadata,
}

#[derive(Debug)]
pub struct MediaIngestor<R, T, P> {
    fetcher: MediaFetcher<R, T>,
    probe: P,
}

impl<R, T, P> MediaIngestor<R, T, P>
where
    R: DnsResolver,
    T: HttpTransport,
    P: MediaProbe,
{
    #[must_use]
    pub const fn new(fetcher: MediaFetcher<R, T>, probe: P) -> Self {
        Self { fetcher, probe }
    }

    /// Downloads then probes one object. A failed probe removes the completed
    /// cache object so unrecognized bytes are not retained. Cancellation after
    /// the download commits but before the probe returns leaves an unleased,
    /// quota-accounted object for the bounded janitor to reclaim; it never
    /// converts committed bytes back into an unmetered reservation.
    ///
    /// # Errors
    ///
    /// Returns [`IngestError`] for fetch, inspection, or cleanup failure.
    pub async fn ingest(&self, request: MediaRequest) -> Result<InspectedMedia, IngestError> {
        let download = self.fetcher.fetch(request).await?;
        match self.probe.probe(&download.path).await {
            Ok(metadata) => Ok(InspectedMedia { download, metadata }),
            Err(error) => {
                self.fetcher
                    .discard(&download.path)
                    .await
                    .map_err(IngestError::Cleanup)?;
                Err(IngestError::Probe(error))
            }
        }
    }

    /// Removes a completed object through the same deletion permit and hard
    /// capacity ledger used by the janitor.
    ///
    /// # Errors
    ///
    /// Returns [`IngestError::Cleanup`] when the target cannot be proven safe,
    /// is actively leased, or cannot be removed.
    pub async fn discard(&self, path: &Path) -> Result<(), IngestError> {
        self.fetcher
            .discard(path)
            .await
            .map_err(IngestError::Cleanup)
    }
}
