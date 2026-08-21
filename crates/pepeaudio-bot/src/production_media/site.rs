use super::{
    ProductionMediaResolver,
    site_batch::{
        CompletedTracks, acquire_until, admission_deadline, classify_item, run_until_error,
    },
};
use crate::{ResolveError, ResolvedMediaBatch};
use pepeaudio_core::UserId;

impl ProductionMediaResolver {
    pub(super) async fn resolve_query(
        &self,
        query: &str,
        requester: UserId,
    ) -> Result<ResolvedMediaBatch, ResolveError> {
        let _batch_permit = self
            .site_batches
            .try_acquire()
            .map_err(|_| ResolveError::Busy)?;
        let client = self
            .site_client
            .as_ref()
            .ok_or(ResolveError::SiteExtractorsDisabled)?;
        let deadline = admission_deadline();
        let track = tokio::time::timeout_at(deadline, async {
            let _permit = acquire_until(&self.site_admission, deadline).await?;
            let resolved = client
                .resolve_query(query)
                .await
                .map_err(|error| map_site_error(&error))?;
            self.ingest_resolved_site(resolved, requester).await
        })
        .await
        .map_err(|_| ResolveError::TimedOut)??;
        Ok(ResolvedMediaBatch::single(track))
    }

    pub(super) async fn resolve_site(
        &self,
        raw_url: &str,
        requester: UserId,
        maximum_items: usize,
    ) -> Result<ResolvedMediaBatch, ResolveError> {
        let _batch_permit = self
            .site_batches
            .try_acquire()
            .map_err(|_| ResolveError::Busy)?;
        let client = self
            .site_client
            .as_ref()
            .ok_or(ResolveError::SiteExtractorsDisabled)?;
        let deadline = admission_deadline();
        if client
            .is_single_item_url(raw_url)
            .map_err(|error| map_site_error(&error))?
        {
            let track = tokio::time::timeout_at(deadline, async {
                let _permit = acquire_until(&self.site_admission, deadline).await?;
                let resolved = client
                    .resolve_page(raw_url)
                    .await
                    .map_err(|error| map_site_error(&error))?;
                self.ingest_resolved_site(resolved, requester).await
            })
            .await
            .map_err(|_| ResolveError::TimedOut)??;
            return Ok(ResolvedMediaBatch::single(track));
        }
        let collection = tokio::time::timeout_at(
            deadline,
            client.discover_url(raw_url, maximum_items.min(self.maximum_playlist_items)),
        )
        .await
        .map_err(|_| ResolveError::TimedOut)?
        .map_err(|error| map_site_error(&error))?;
        let completed = CompletedTracks::with_capacity(collection.entries.len());
        let work = run_until_error(collection.entries, |index, reference| {
            let completed = completed.clone();
            async move {
                let result = async {
                    let _permit = acquire_until(&self.site_admission, deadline).await?;
                    let resolved = client
                        .resolve(&reference)
                        .await
                        .map_err(|error| map_site_error(&error))?;
                    let metadata = super::metadata::from_site_track(&resolved)?;
                    let title = resolved.title;
                    let track = self
                        .ingest(
                            &self.site_ingestor,
                            resolved.request,
                            requester,
                            Some(&title),
                            self.maximum_site_bytes,
                        )
                        .await?
                        .with_metadata(metadata);
                    completed.register(index, track);
                    Ok::<_, ResolveError>(())
                }
                .await;
                classify_item(result)
            }
        });
        let Ok(outcome) = tokio::time::timeout_at(deadline, work).await else {
            if self.discard_tracks(completed.take()).await.is_err() {
                tracing::warn!("timed-out site batch cleanup remains pending for the janitor");
            }
            return Err(ResolveError::TimedOut);
        };

        if let Some(error) = outcome.fatal_error {
            if self.discard_tracks(completed.take()).await.is_err() {
                tracing::warn!("rejected site batch cleanup remains pending for the janitor");
            }
            return Err(error);
        }
        let tracks = completed.take_ordered();
        if tracks.is_empty() {
            return Err(outcome
                .first_skipped_error
                .unwrap_or(ResolveError::UnsupportedStream));
        }
        Ok(ResolvedMediaBatch {
            tracks,
            source_title: collection.title,
            source_item_count: collection.source_item_count,
            skipped_items: collection
                .skipped_items
                .saturating_add(outcome.skipped_items),
            truncated: collection.truncated,
        })
    }

    async fn ingest_resolved_site(
        &self,
        resolved: pepeaudio_media::SiteResolvedTrack,
        requester: UserId,
    ) -> Result<pepeaudio_player::QueueTrack, ResolveError> {
        let metadata = super::metadata::from_site_track(&resolved)?;
        let title = resolved.title;
        Ok(self
            .ingest(
                &self.site_ingestor,
                resolved.request,
                requester,
                Some(&title),
                self.maximum_site_bytes,
            )
            .await?
            .with_metadata(metadata))
    }
}

pub(super) fn map_site_error(error: &pepeaudio_media::SiteError) -> ResolveError {
    match error {
        pepeaudio_media::SiteError::PlaylistTooLarge { .. } => ResolveError::PlaylistTooLarge,
        pepeaudio_media::SiteError::UnsupportedStream => ResolveError::UnsupportedStream,
        pepeaudio_media::SiteError::NoSearchMatch => ResolveError::NoSearchMatch,
        pepeaudio_media::SiteError::DurationLimit => ResolveError::TrackLimitExceeded,
        pepeaudio_media::SiteError::InvalidUrl => ResolveError::UnsupportedUrl,
        _ => ResolveError::Failed("site media resolution failed".into()),
    }
}
