use std::path::PathBuf;

use pepeaudio_core::UserId;
use pepeaudio_media::{InspectedMedia, MediaRequest};
use pepeaudio_player::{PlaybackSource, QueueTrack};

use super::{Ingestor, ProductionMediaResolver, validation};
use crate::ResolveError;

impl ProductionMediaResolver {
    pub(super) async fn ingest(
        &self,
        ingestor: &Ingestor,
        request: MediaRequest,
        requester: UserId,
        preferred_title: Option<&str>,
        maximum_bytes: u64,
    ) -> Result<QueueTrack, ResolveError> {
        let source_kind = request.source_kind();
        let inspected = self
            .ingest_with_reclaim(ingestor, request, maximum_bytes)
            .await
            .map_err(|error| {
                self.log_capacity_rejection(&error);
                validation::map_ingest(&error, source_kind)
            })?;
        let duration_ms = match validation::duration_ms(inspected.metadata.duration_seconds) {
            Ok(duration) => duration,
            Err(error) => {
                self.discard_download(inspected.download.path.clone())
                    .await?;
                return Err(error);
            }
        };
        if !validation::supported_format(inspected.metadata.format_name.as_deref()) {
            self.discard_download(inspected.download.path.clone())
                .await?;
            return Err(validation::unsupported_media(source_kind));
        }
        if duration_ms.is_none_or(|duration| {
            duration > u64::try_from(self.maximum_duration.as_millis()).unwrap_or(u64::MAX)
        }) {
            self.discard_download(inspected.download.path.clone())
                .await?;
            return Err(ResolveError::TrackLimitExceeded);
        }
        self.leased_track(inspected, requester, preferred_title, duration_ms)
            .await
    }

    async fn leased_track(
        &self,
        inspected: InspectedMedia,
        requester: UserId,
        preferred_title: Option<&str>,
        duration_ms: Option<u64>,
    ) -> Result<QueueTrack, ResolveError> {
        let Ok(lease) = self.media_leases.acquire(&inspected.download.path).await else {
            let _ = self.discard_download(inspected.download.path.clone()).await;
            return Err(ResolveError::Failed(
                "managed media lease could not be acquired".into(),
            ));
        };
        let Some(source) = lease.canonical_path().to_str().map(str::to_owned) else {
            drop(lease);
            self.discard_download(inspected.download.path.clone())
                .await?;
            return Err(ResolveError::Failed(
                "managed media path is not representable".into(),
            ));
        };
        let title = validation::display_title(preferred_title, &inspected.download.final_url);
        Ok(QueueTrack::new(
            title,
            Some(requester),
            duration_ms,
            true,
            PlaybackSource::with_lease(source, lease),
        ))
    }

    async fn ingest_with_reclaim(
        &self,
        ingestor: &Ingestor,
        request: MediaRequest,
        maximum_bytes: u64,
    ) -> Result<InspectedMedia, pepeaudio_media::IngestError> {
        let reclaim_bytes = match &request {
            MediaRequest::DiscordAttachment(attachment) => attachment
                .declared_size_bytes
                .unwrap_or(maximum_bytes)
                .max(1),
            MediaRequest::DirectUrl { .. } | MediaRequest::ResolvedSite(_) => maximum_bytes,
        };
        match ingestor.ingest(request.clone()).await {
            Err(error) if validation::is_admission_capacity_error(&error) => {
                if let Ok(report) = self.media_janitor.run_for_admission(reclaim_bytes).await
                    && !report.removals.is_empty()
                {
                    tracing::info!(
                        removed = report.removals.len(),
                        "on-demand media cleanup reclaimed quota candidates"
                    );
                }
                ingestor.ingest(request).await
            }
            result => result,
        }
    }

    pub(super) async fn discard_tracks(&self, tracks: Vec<QueueTrack>) -> Result<(), ResolveError> {
        let mut first_error = None;
        for track in tracks {
            let candidate = PathBuf::from(track.source.as_str());
            drop(track);
            if let Err(error) = self.discard_download(candidate).await
                && first_error.is_none()
            {
                first_error = Some(error);
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    fn log_capacity_rejection(&self, error: &pepeaudio_media::IngestError) {
        if !validation::is_capacity_error(error) {
            return;
        }
        if let Some(usage) = self.media_leases.capacity_usage() {
            tracing::warn!(
                used_bytes = usage.used_bytes,
                reserved_bytes = usage.reserved_bytes,
                maximum_bytes = usage.maximum_bytes,
                managed_files = usage.managed_files,
                reservations = usage.reservations,
                "managed media hard quota rejected an ingest"
            );
        }
    }
}
