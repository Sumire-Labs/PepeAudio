use std::sync::Arc;

use async_trait::async_trait;
use url::Url;

use super::{
    metadata::{clean_text, storefront},
    public_wire::{LookupResponse, LookupResult},
};
use crate::{
    CatalogCollection, CatalogError, CatalogItemKind, CatalogProvider, CatalogReference,
    CatalogResult, CatalogTrackMetadata,
    http::{HttpError, HttpRequest, ReqwestTransport, SharedTransport},
    provider::ProviderCatalog,
};

const LOOKUP_ENDPOINT: &str = "https://itunes.apple.com/lookup";
const MAX_PUBLIC_RESPONSE_BYTES: usize = 1024 * 1024;
const HARD_COLLECTION_LIMIT: usize = 100;
const MAX_DURATION_MS: u64 = 7 * 24 * 60 * 60 * 1_000;

pub struct AppleMusicPublicCatalog {
    transport: SharedTransport,
}

impl AppleMusicPublicCatalog {
    /// Creates a credential-free, metadata-only Apple catalog client.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogError::Transport`] if the bounded HTTP client cannot
    /// be constructed.
    pub fn new() -> CatalogResult<Self> {
        let transport = Arc::new(
            ReqwestTransport::new()
                .map_err(|_| CatalogError::Transport(CatalogProvider::AppleMusic))?,
        );
        Ok(Self::with_transport(transport))
    }

    fn with_transport(transport: SharedTransport) -> Self {
        Self { transport }
    }

    async fn resolve_track(
        &self,
        reference: &CatalogReference,
    ) -> CatalogResult<CatalogCollection> {
        let storefront = storefront(reference)?;
        let expected_id = numeric_id(reference.id())?;
        let response = self
            .get_lookup(Self::lookup_url(reference.id(), storefront, None))
            .await?;
        if response.result_count == 0 {
            return Err(CatalogError::NotFound(CatalogProvider::AppleMusic));
        }
        let mut matches = response.results.into_iter().filter(|item| {
            item.wrapper_type.as_deref() == Some("track")
                && item.kind.as_deref() == Some("song")
                && item.track_id == Some(expected_id)
        });
        let item = matches
            .next()
            .ok_or(CatalogError::InvalidResponse(CatalogProvider::AppleMusic))?;
        if matches.next().is_some() {
            return Err(CatalogError::InvalidResponse(CatalogProvider::AppleMusic));
        }
        let metadata = track_metadata(&item, reference.clone())
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
        let expected_id = numeric_id(reference.id())?;
        let lookup_track_limit = limit.saturating_add(1).min(200);
        let response = self
            .get_lookup(Self::lookup_url(
                reference.id(),
                storefront,
                Some(lookup_track_limit),
            ))
            .await?;
        if response.result_count == 0 {
            return Err(CatalogError::NotFound(CatalogProvider::AppleMusic));
        }

        let title = album_title(&response.results, expected_id)?;
        let source_items = response
            .results
            .into_iter()
            .filter(|item| item.wrapper_type.as_deref() != Some("collection"))
            .collect::<Vec<_>>();
        let returned_items = source_items.len();
        let reported_total = consistent_track_count(&source_items);
        let mut tracks = Vec::with_capacity(returned_items.min(limit));
        let mut skipped = 0;
        for item in source_items.into_iter().take(limit) {
            let Some(track_id) = album_track_id(&item, expected_id) else {
                skipped += 1;
                continue;
            };
            let track_reference = album_track_reference(reference, track_id);
            match track_metadata(&item, track_reference) {
                Some(metadata) => tracks.push(metadata),
                None => skipped += 1,
            }
        }
        let response_may_be_limited = returned_items >= lookup_track_limit;
        let source_item_count = reported_total.or_else(|| {
            (!response_may_be_limited && returned_items <= limit).then_some(returned_items)
        });
        let truncated = returned_items > limit
            || reported_total.is_some_and(|count| count > limit)
            || (response_may_be_limited && reported_total.is_none());
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

    fn lookup_url(id: &str, storefront: &str, album_limit: Option<usize>) -> Url {
        let mut url = Url::parse(LOOKUP_ENDPOINT).expect("constant iTunes Lookup endpoint");
        let mut query = url.query_pairs_mut();
        query.append_pair("id", id);
        query.append_pair("country", &storefront.to_ascii_uppercase());
        if let Some(limit) = album_limit {
            query.append_pair("entity", "song");
            query.append_pair("limit", &limit.to_string());
        }
        drop(query);
        url
    }

    async fn get_lookup(&self, url: Url) -> CatalogResult<LookupResponse> {
        let request = HttpRequest::get(url)
            .with_header("accept", "application/json".to_owned())
            .with_response_limit(MAX_PUBLIC_RESPONSE_BYTES);
        let response = self
            .transport
            .execute(request)
            .await
            .map_err(map_http_error)?;
        match response.status {
            200 => {
                let decoded: LookupResponse = serde_json::from_slice(&response.body)
                    .map_err(|_| CatalogError::InvalidResponse(CatalogProvider::AppleMusic))?;
                if decoded.result_count != decoded.results.len() {
                    return Err(CatalogError::InvalidResponse(CatalogProvider::AppleMusic));
                }
                Ok(decoded)
            }
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
impl ProviderCatalog for AppleMusicPublicCatalog {
    async fn resolve(
        &self,
        reference: &CatalogReference,
        collection_limit: usize,
    ) -> CatalogResult<CatalogCollection> {
        if reference.provider() != CatalogProvider::AppleMusic {
            return Err(CatalogError::UnsupportedUrl);
        }
        if !(1..=HARD_COLLECTION_LIMIT).contains(&collection_limit) {
            return Err(CatalogError::InvalidCollectionLimit);
        }
        match reference.kind() {
            CatalogItemKind::Track => self.resolve_track(reference).await,
            CatalogItemKind::Album => self.resolve_album(reference, collection_limit).await,
            CatalogItemKind::Playlist => Err(CatalogError::AppleMusicPlaylistRequiresCredentials),
        }
    }
}

fn numeric_id(value: &str) -> CatalogResult<u64> {
    value.parse().map_err(|_| CatalogError::UnsupportedUrl)
}

fn album_title(results: &[LookupResult], expected_id: u64) -> CatalogResult<String> {
    let mut titles = results.iter().filter_map(|item| {
        if item.wrapper_type.as_deref() == Some("collection")
            && item.collection_type.as_deref() == Some("Album")
            && item.collection_id == Some(expected_id)
        {
            item.collection_name
                .as_deref()
                .and_then(|name| clean_text(name, 512))
        } else {
            None
        }
    });
    let title = titles
        .next()
        .ok_or(CatalogError::InvalidResponse(CatalogProvider::AppleMusic))?;
    if titles.next().is_some() {
        return Err(CatalogError::InvalidResponse(CatalogProvider::AppleMusic));
    }
    Ok(title)
}

fn album_track_id(item: &LookupResult, expected_album_id: u64) -> Option<u64> {
    (item.wrapper_type.as_deref() == Some("track")
        && item.kind.as_deref() == Some("song")
        && item.collection_id == Some(expected_album_id))
    .then_some(item.track_id?)
}

fn album_track_reference(album_reference: &CatalogReference, track_id: u64) -> CatalogReference {
    let mut canonical_url = album_reference.canonical_url().clone();
    canonical_url
        .query_pairs_mut()
        .append_pair("i", &track_id.to_string());
    CatalogReference::new(
        CatalogProvider::AppleMusic,
        CatalogItemKind::Track,
        track_id.to_string(),
        album_reference.storefront().map(str::to_owned),
        canonical_url,
    )
}

fn track_metadata(
    item: &LookupResult,
    reference: CatalogReference,
) -> Option<CatalogTrackMetadata> {
    let title = clean_text(item.track_name.as_deref()?, 512)?;
    let artist = clean_text(item.artist_name.as_deref()?, 256)?;
    Some(CatalogTrackMetadata {
        reference,
        title,
        artists: vec![artist],
        album: item
            .collection_name
            .as_deref()
            .and_then(|name| clean_text(name, 512)),
        duration_ms: item
            .track_time_millis
            .filter(|duration| *duration <= MAX_DURATION_MS),
        isrc: None,
    })
}

fn consistent_track_count(items: &[LookupResult]) -> Option<usize> {
    let mut counts = items.iter().map(|item| item.track_count);
    let first = counts.next().flatten()?;
    (first >= items.len() && counts.all(|count| count == Some(first))).then_some(first)
}

fn map_http_error(error: HttpError) -> CatalogError {
    match error {
        HttpError::Transport => CatalogError::Transport(CatalogProvider::AppleMusic),
        HttpError::ResponseTooLarge => CatalogError::ResponseTooLarge(CatalogProvider::AppleMusic),
    }
}

#[cfg(test)]
#[path = "public_tests.rs"]
mod tests;
