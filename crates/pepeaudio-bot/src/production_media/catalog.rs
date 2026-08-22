use pepeaudio_catalog::{
    CatalogError, CatalogItemKind, CatalogProvider, CatalogReference, CatalogSearchRequest,
    CatalogTrackMetadata,
};
use pepeaudio_core::UserId;
use pepeaudio_media::SiteSearch;

use super::{
    ProductionMediaResolver,
    site::map_site_error,
    site_batch::{
        CompletedTracks, acquire_until, admission_deadline, classify_item, run_until_error,
    },
};
use crate::{ResolveError, ResolvedMediaBatch};

const SITE_SEARCH_TEXT_BYTES: usize = 200;
const SITE_SEARCH_ARTISTS: usize = 16;

impl ProductionMediaResolver {
    pub(super) async fn resolve_catalog(
        &self,
        reference: CatalogReference,
        requester: UserId,
        maximum_items: usize,
    ) -> Result<ResolvedMediaBatch, ResolveError> {
        let _batch_permit = self
            .site_batches
            .try_acquire()
            .map_err(|_| ResolveError::Busy)?;
        let resolver = self
            .catalog_resolver
            .as_ref()
            .ok_or(ResolveError::CrossServiceMatchingDisabled)?;
        if reference.provider() == CatalogProvider::Spotify
            && reference.kind() == CatalogItemKind::Playlist
        {
            return Err(ResolveError::SpotifyPlaylistRequiresUserAuthorization);
        }
        let client = self
            .site_client
            .as_ref()
            .ok_or(ResolveError::SiteExtractorsDisabled)?;
        let deadline = admission_deadline();
        let local_limit = maximum_items
            .min(self.maximum_playlist_items)
            .min(resolver.collection_limit());
        if local_limit == 0 {
            return Err(ResolveError::CapacityExceeded);
        }
        let mut collection = tokio::time::timeout_at(
            deadline,
            resolver.resolve_reference_with_limit(&reference, local_limit),
        )
        .await
        .map_err(|_| ResolveError::TimedOut)?
        .map_err(map_catalog_error)?;
        let locally_truncated = collection.tracks.len() > local_limit;
        collection.tracks.truncate(local_limit);
        if collection.tracks.is_empty() {
            return Err(ResolveError::UnsupportedStream);
        }

        let completed = CompletedTracks::with_capacity(collection.tracks.len());
        let work = run_until_error(collection.tracks, |index, catalog_track| {
            let completed = completed.clone();
            async move {
                let result = async {
                    let _permit = acquire_until(&self.site_admission, deadline).await?;
                    let search = site_search(&catalog_track)?;
                    let site_track = client
                        .resolve_search(&search)
                        .await
                        .map_err(|error| map_site_error(&error))?;
                    let metadata =
                        super::metadata::from_catalog_track(&catalog_track, &site_track)?;
                    let track = self
                        .ingest(
                            &self.site_ingestor,
                            site_track.request,
                            requester,
                            Some(&catalog_track.title),
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
                tracing::warn!("timed-out catalog batch cleanup remains pending for the janitor");
            }
            return Err(ResolveError::TimedOut);
        };
        if let Some(error) = outcome.fatal_error {
            if self.discard_tracks(completed.take()).await.is_err() {
                tracing::warn!("rejected catalog batch cleanup remains pending for the janitor");
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
            source_title: Some(collection.title),
            source_item_count: collection.source_item_count,
            skipped_items: collection
                .skipped_item_count
                .saturating_add(outcome.skipped_items),
            truncated: collection.truncated || locally_truncated,
        })
    }
}

fn site_search(track: &CatalogTrackMetadata) -> Result<SiteSearch, ResolveError> {
    let request = track.search_request();
    SiteSearch::new(
        request.text_query(),
        bounded_text(&request.title, SITE_SEARCH_TEXT_BYTES),
        bounded_artists(&request),
        request.duration_ms,
        request.isrc,
    )
    .map_err(|error| map_site_error(&error))
}

fn bounded_artists(request: &CatalogSearchRequest) -> Vec<String> {
    request
        .artists
        .iter()
        .take(SITE_SEARCH_ARTISTS)
        .map(|artist| bounded_text(artist, SITE_SEARCH_TEXT_BYTES))
        .filter(|artist| !artist.is_empty())
        .collect()
}

fn bounded_text(value: &str, maximum_bytes: usize) -> String {
    let mut output = String::with_capacity(value.len().min(maximum_bytes));
    for character in value.chars().filter(|character| !character.is_control()) {
        if output.len() + character.len_utf8() > maximum_bytes {
            break;
        }
        output.push(character);
    }
    output.trim().to_owned()
}

pub(super) fn map_catalog_error(error: CatalogError) -> ResolveError {
    match error {
        CatalogError::UnsupportedUrl => ResolveError::UnsupportedUrl,
        CatalogError::ProviderNotConfigured(_) => ResolveError::CatalogProviderUnavailable,
        CatalogError::SpotifyPlaylistAccessDenied => {
            ResolveError::SpotifyPlaylistRequiresUserAuthorization
        }
        CatalogError::PublicMetadataUnsupported {
            provider: CatalogProvider::Spotify,
            kind: CatalogItemKind::Album,
        } => ResolveError::SpotifyAlbumRequiresCredentials,
        CatalogError::PublicMetadataUnsupported {
            provider: CatalogProvider::Spotify,
            kind: CatalogItemKind::Playlist,
        } => ResolveError::SpotifyPlaylistRequiresUserAuthorization,
        CatalogError::AppleMusicPlaylistRequiresCredentials => {
            ResolveError::AppleMusicPlaylistRequiresDeveloperCredentials
        }
        CatalogError::PublicMetadataUnsupported {
            provider: CatalogProvider::AppleMusic,
            kind: CatalogItemKind::Playlist,
        } => ResolveError::AppleMusicPlaylistRequiresDeveloperCredentials,
        error => {
            tracing::warn!(error = %error, "catalog metadata resolution failed");
            ResolveError::Failed("catalog metadata resolution failed".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use pepeaudio_catalog::{CatalogError, CatalogItemKind, CatalogProvider};

    use super::{bounded_text, map_catalog_error};
    use crate::ResolveError;

    #[test]
    fn catalog_search_text_is_utf8_safe_at_the_media_boundary() {
        let value = "音楽".repeat(200);
        let bounded = bounded_text(&value, 200);
        assert!(bounded.len() <= 200);
        assert!(value.starts_with(&bounded));
        assert!(!bounded.is_empty());
    }

    #[test]
    fn public_collection_limits_keep_specific_safe_error_types() {
        assert_eq!(
            map_catalog_error(CatalogError::PublicMetadataUnsupported {
                provider: CatalogProvider::Spotify,
                kind: CatalogItemKind::Album,
            }),
            ResolveError::SpotifyAlbumRequiresCredentials
        );
        assert_eq!(
            map_catalog_error(CatalogError::PublicMetadataUnsupported {
                provider: CatalogProvider::Spotify,
                kind: CatalogItemKind::Playlist,
            }),
            ResolveError::SpotifyPlaylistRequiresUserAuthorization
        );
        assert_eq!(
            map_catalog_error(CatalogError::AppleMusicPlaylistRequiresCredentials),
            ResolveError::AppleMusicPlaylistRequiresDeveloperCredentials
        );
    }
}
