use std::{collections::HashSet, sync::Arc};

use async_trait::async_trait;
use serde::de::DeserializeOwned;
use url::Url;

use super::{
    metadata::{clean_text, one_resource, song_metadata, storefront, validated_next_url},
    token::DeveloperToken,
    wire::{AlbumAttributes, PlaylistAttributes, Response, SongAttributes},
};
use crate::{
    CatalogCollection, CatalogError, CatalogItemKind, CatalogProvider, CatalogReference,
    CatalogResult, CatalogTrackMetadata,
    http::{HttpError, HttpRequest, ReqwestTransport, SharedTransport},
    provider::ProviderCatalog,
};

const API_BASE: &str = "https://api.music.apple.com/v1";

pub struct AppleMusicCatalog {
    tokens: DeveloperToken,
    transport: SharedTransport,
}

impl AppleMusicCatalog {
    /// Creates a catalog-only Apple Music client. This client intentionally has
    /// no Music User Token support and cannot access a user's library.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogError::InvalidCredentials`] for malformed identifiers
    /// or a non-PKCS#8 P-256 private key.
    pub fn new(
        team_id: impl Into<String>,
        key_id: impl Into<String>,
        private_key_pem: &str,
    ) -> CatalogResult<Self> {
        let transport: SharedTransport = Arc::new(
            ReqwestTransport::new()
                .map_err(|_| CatalogError::Transport(CatalogProvider::AppleMusic))?,
        );
        Self::with_transport(team_id, key_id, private_key_pem, transport)
    }

    fn with_transport(
        team_id: impl Into<String>,
        key_id: impl Into<String>,
        private_key_pem: &str,
        transport: SharedTransport,
    ) -> CatalogResult<Self> {
        Ok(Self {
            tokens: DeveloperToken::new(team_id, key_id, private_key_pem)?,
            transport,
        })
    }

    async fn resolve_track(
        &self,
        reference: &CatalogReference,
    ) -> CatalogResult<CatalogCollection> {
        let storefront = storefront(reference)?;
        let response: Response<SongAttributes> = self
            .get_json(Self::resource_url(
                storefront,
                "songs",
                reference.id(),
                None,
            ))
            .await?;
        let metadata = one_resource(response, "songs")
            .and_then(|resource| song_metadata(resource, storefront))
            .ok_or(CatalogError::InvalidResponse(CatalogProvider::AppleMusic))?;
        Ok(CatalogCollection {
            reference: reference.clone(),
            title: metadata.title.clone(),
            tracks: vec![metadata],
            source_item_count: Some(1),
            skipped_item_count: 0,
            truncated: false,
            version: None,
        })
    }

    async fn resolve_album(
        &self,
        reference: &CatalogReference,
        limit: usize,
    ) -> CatalogResult<CatalogCollection> {
        let storefront = storefront(reference)?;
        let response: Response<AlbumAttributes> = self
            .get_json(Self::resource_url(
                storefront,
                "albums",
                reference.id(),
                None,
            ))
            .await?;
        let title = one_resource(response, "albums")
            .and_then(|resource| resource.attributes)
            .and_then(|attributes| clean_text(&attributes.name, 512))
            .ok_or(CatalogError::InvalidResponse(CatalogProvider::AppleMusic))?;
        let tracks_url = Self::resource_url(storefront, "albums", reference.id(), Some("tracks"));
        let tracks = self
            .resolve_track_pages(tracks_url, storefront, limit)
            .await?;
        Ok(CatalogCollection {
            reference: reference.clone(),
            title,
            tracks: tracks.items,
            source_item_count: tracks.total,
            skipped_item_count: tracks.skipped,
            truncated: tracks.truncated,
            version: None,
        })
    }

    async fn resolve_playlist(
        &self,
        reference: &CatalogReference,
        limit: usize,
    ) -> CatalogResult<CatalogCollection> {
        let storefront = storefront(reference)?;
        let response: Response<PlaylistAttributes> = self
            .get_json(Self::resource_url(
                storefront,
                "playlists",
                reference.id(),
                None,
            ))
            .await?;
        let attributes = one_resource(response, "playlists")
            .and_then(|resource| resource.attributes)
            .ok_or(CatalogError::InvalidResponse(CatalogProvider::AppleMusic))?;
        let title = clean_text(&attributes.name, 512)
            .ok_or(CatalogError::InvalidResponse(CatalogProvider::AppleMusic))?;
        let version = attributes
            .last_modified_date
            .filter(|value| !value.is_empty() && value.len() <= 64);
        let tracks_url =
            Self::resource_url(storefront, "playlists", reference.id(), Some("tracks"));
        let tracks = self
            .resolve_track_pages(tracks_url, storefront, limit)
            .await?;
        Ok(CatalogCollection {
            reference: reference.clone(),
            title,
            tracks: tracks.items,
            source_item_count: tracks.total,
            skipped_item_count: tracks.skipped,
            truncated: tracks.truncated,
            version,
        })
    }

    async fn resolve_track_pages(
        &self,
        mut next_url: Url,
        storefront: &str,
        limit: usize,
    ) -> CatalogResult<TrackPageResult> {
        next_url
            .query_pairs_mut()
            .append_pair("limit", &limit.to_string());
        let expected_path = next_url.path().to_owned();
        let mut pending = Some(next_url);
        let mut seen = HashSet::new();
        let mut items = Vec::new();
        let mut skipped = 0;
        let mut processed = 0;
        let mut total = None;
        let mut current_page_truncated = false;
        while processed < limit {
            let Some(url) = pending.take() else {
                break;
            };
            if !seen.insert(url.to_string()) {
                return Err(CatalogError::InvalidResponse(CatalogProvider::AppleMusic));
            }
            let page: Response<SongAttributes> = self.get_json(url).await?;
            if total.is_none() {
                total = page.meta.and_then(|meta| meta.total);
            }
            let page_length = page.data.len();
            let remaining = limit - processed;
            current_page_truncated = page_length > remaining;
            for resource in page.data.into_iter().take(remaining) {
                processed += 1;
                match song_metadata(resource, storefront) {
                    Some(track) => items.push(track),
                    None => skipped += 1,
                }
            }
            pending = page
                .next
                .map(|value| validated_next_url(&value, storefront, &expected_path))
                .transpose()?;
            if page_length == 0 && pending.is_some() {
                return Err(CatalogError::InvalidResponse(CatalogProvider::AppleMusic));
            }
        }
        let source_total = total;
        let truncated = current_page_truncated
            || pending.is_some()
            || source_total.is_some_and(|count| count > processed);
        Ok(TrackPageResult {
            items,
            total: source_total,
            skipped,
            truncated,
        })
    }

    fn resource_url(storefront: &str, resource: &str, id: &str, relationship: Option<&str>) -> Url {
        let mut url = Url::parse(API_BASE).expect("constant Apple Music API base");
        let mut segments = url.path_segments_mut().expect("API URL can be a base");
        segments
            .push("catalog")
            .push(storefront)
            .push(resource)
            .push(id);
        if let Some(relationship) = relationship {
            segments.push(relationship);
        }
        drop(segments);
        url
    }

    async fn get_json<T: DeserializeOwned>(&self, url: Url) -> CatalogResult<T> {
        let token = self.tokens.token().await?;
        let response = self
            .transport
            .execute(HttpRequest::get(url).with_header("authorization", format!("Bearer {token}")))
            .await
            .map_err(map_http_error)?;
        match response.status {
            200 => serde_json::from_slice(&response.body)
                .map_err(|_| CatalogError::InvalidResponse(CatalogProvider::AppleMusic)),
            401 => Err(CatalogError::InvalidCredentials(
                CatalogProvider::AppleMusic,
            )),
            403 => Err(CatalogError::AccessDenied(CatalogProvider::AppleMusic)),
            404 => Err(CatalogError::NotFound(CatalogProvider::AppleMusic)),
            429 => Err(CatalogError::RateLimited {
                provider: CatalogProvider::AppleMusic,
                retry_after_seconds: response.retry_after_seconds,
            }),
            _ => Err(CatalogError::Transport(CatalogProvider::AppleMusic)),
        }
    }
}

#[async_trait]
impl ProviderCatalog for AppleMusicCatalog {
    async fn resolve(
        &self,
        reference: &CatalogReference,
        collection_limit: usize,
    ) -> CatalogResult<CatalogCollection> {
        if reference.provider() != CatalogProvider::AppleMusic {
            return Err(CatalogError::UnsupportedUrl);
        }
        match reference.kind() {
            CatalogItemKind::Track => self.resolve_track(reference).await,
            CatalogItemKind::Album => self.resolve_album(reference, collection_limit).await,
            CatalogItemKind::Playlist => self.resolve_playlist(reference, collection_limit).await,
        }
    }
}

struct TrackPageResult {
    items: Vec<CatalogTrackMetadata>,
    total: Option<usize>,
    skipped: usize,
    truncated: bool,
}

fn map_http_error(error: HttpError) -> CatalogError {
    match error {
        HttpError::Transport => CatalogError::Transport(CatalogProvider::AppleMusic),
        HttpError::ResponseTooLarge => CatalogError::ResponseTooLarge(CatalogProvider::AppleMusic),
    }
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
