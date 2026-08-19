use std::sync::Arc;

use async_trait::async_trait;
use serde::de::DeserializeOwned;
use url::Url;

use super::{
    auth::TokenManager,
    metadata::{clean_text, simplified_track_metadata, track_metadata, valid_market},
    wire::{Album, Track},
};
use crate::{
    CatalogCollection, CatalogError, CatalogItemKind, CatalogProvider, CatalogReference,
    CatalogResult,
    http::{HttpError, HttpRequest, HttpResponse, ReqwestTransport, SharedTransport},
    provider::ProviderCatalog,
};

const API_BASE: &str = "https://api.spotify.com/v1";
const PAGE_SIZE: usize = 50;

pub struct SpotifyCatalog {
    market: String,
    tokens: TokenManager,
    transport: SharedTransport,
}

impl SpotifyCatalog {
    /// Creates an app-only Spotify catalog client using the client credentials
    /// flow. No Spotify user token or cookie is accepted.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogError::InvalidCredentials`] for empty credentials or an
    /// invalid market, and [`CatalogError::Transport`] if the HTTP client cannot
    /// be initialized.
    pub fn new(
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
        market: impl Into<String>,
    ) -> CatalogResult<Self> {
        let transport: SharedTransport = Arc::new(
            ReqwestTransport::new()
                .map_err(|_| CatalogError::Transport(CatalogProvider::Spotify))?,
        );
        Self::with_transport(client_id, client_secret, market, transport)
    }

    fn with_transport(
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
        market: impl Into<String>,
        transport: SharedTransport,
    ) -> CatalogResult<Self> {
        let market = market.into();
        if !valid_market(&market) {
            return Err(CatalogError::InvalidCredentials(CatalogProvider::Spotify));
        }
        let tokens = TokenManager::new(client_id, client_secret, Arc::clone(&transport))?;
        Ok(Self {
            market,
            tokens,
            transport,
        })
    }

    async fn resolve_track(
        &self,
        reference: &CatalogReference,
    ) -> CatalogResult<CatalogCollection> {
        let url = self.api_url("tracks", reference.id(), None, None);
        let track: Track = self.get_json(url).await?;
        let metadata = track_metadata(track, None)
            .ok_or(CatalogError::InvalidResponse(CatalogProvider::Spotify))?;
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
        let url = self.api_url("albums", reference.id(), None, None);
        let album: Album = self.get_json(url).await?;
        let title = clean_text(&album.name, 512)
            .ok_or(CatalogError::InvalidResponse(CatalogProvider::Spotify))?;
        let total = album.tracks.total;
        let mut tracks = Vec::new();
        let mut skipped = 0;
        let mut processed = 0;
        let mut page = album.tracks;
        let truncated_by_page = loop {
            let had_next = page.next.is_some();
            let page_length = page.items.len();
            let remaining = limit.saturating_sub(processed);
            for item in page.items.into_iter().take(remaining) {
                processed += 1;
                match simplified_track_metadata(item, &title) {
                    Some(track) => tracks.push(track),
                    None => skipped += 1,
                }
            }
            if processed >= limit || !had_next {
                break page_length > remaining || (processed >= limit && had_next);
            }
            if page_length == 0 {
                return Err(CatalogError::InvalidResponse(CatalogProvider::Spotify));
            }
            page = self
                .get_json(self.api_url("albums", reference.id(), Some("tracks"), Some(processed)))
                .await?;
        };
        let source_item_count = total;
        let truncated =
            truncated_by_page || source_item_count.is_some_and(|count| count > processed);
        Ok(CatalogCollection {
            reference: reference.clone(),
            title,
            tracks,
            source_item_count,
            skipped_item_count: skipped,
            truncated,
            version: None,
        })
    }

    fn api_url(
        &self,
        resource: &str,
        id: &str,
        relationship: Option<&str>,
        offset: Option<usize>,
    ) -> Url {
        let mut url = Url::parse(API_BASE).expect("constant Spotify API base");
        {
            let mut segments = url.path_segments_mut().expect("API URL can be a base");
            segments.push(resource).push(id);
            if let Some(relationship) = relationship {
                segments.push(relationship);
            }
        }
        let mut query = url.query_pairs_mut();
        query.append_pair("market", &self.market);
        if offset.is_some() {
            query.append_pair("limit", &PAGE_SIZE.to_string());
        }
        if let Some(offset) = offset {
            query.append_pair("offset", &offset.to_string());
        }
        drop(query);
        url
    }

    async fn get_json<T: DeserializeOwned>(&self, url: Url) -> CatalogResult<T> {
        let mut response = self.authorized_get(url.clone()).await?;
        if response.status == 401 {
            self.tokens.invalidate().await;
            response = self.authorized_get(url).await?;
        }
        match response.status {
            200 => serde_json::from_slice(&response.body)
                .map_err(|_| CatalogError::InvalidResponse(CatalogProvider::Spotify)),
            401 => Err(CatalogError::InvalidCredentials(CatalogProvider::Spotify)),
            403 => Err(CatalogError::AccessDenied(CatalogProvider::Spotify)),
            404 => Err(CatalogError::NotFound(CatalogProvider::Spotify)),
            429 => Err(CatalogError::RateLimited {
                provider: CatalogProvider::Spotify,
                retry_after_seconds: response.retry_after_seconds,
            }),
            _ => Err(CatalogError::Transport(CatalogProvider::Spotify)),
        }
    }

    async fn authorized_get(&self, url: Url) -> CatalogResult<HttpResponse> {
        let token = self.tokens.token().await?;
        self.transport
            .execute(HttpRequest::get(url).with_header("authorization", format!("Bearer {token}")))
            .await
            .map_err(map_http_error)
    }
}

#[async_trait]
impl ProviderCatalog for SpotifyCatalog {
    async fn resolve(
        &self,
        reference: &CatalogReference,
        collection_limit: usize,
    ) -> CatalogResult<CatalogCollection> {
        if reference.provider() != CatalogProvider::Spotify {
            return Err(CatalogError::UnsupportedUrl);
        }
        match reference.kind() {
            CatalogItemKind::Track => self.resolve_track(reference).await,
            CatalogItemKind::Album => self.resolve_album(reference, collection_limit).await,
            CatalogItemKind::Playlist => Err(CatalogError::SpotifyPlaylistAccessDenied),
        }
    }
}

fn map_http_error(error: HttpError) -> CatalogError {
    match error {
        HttpError::Transport => CatalogError::Transport(CatalogProvider::Spotify),
        HttpError::ResponseTooLarge => CatalogError::ResponseTooLarge(CatalogProvider::Spotify),
    }
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
