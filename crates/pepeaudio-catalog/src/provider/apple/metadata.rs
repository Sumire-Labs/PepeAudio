use url::Url;

use super::wire::{Resource, Response, SongAttributes};
use crate::{
    CatalogError, CatalogItemKind, CatalogProvider, CatalogReference, CatalogResult,
    CatalogTrackMetadata, parse_catalog_url,
};

const API_BASE: &str = "https://api.music.apple.com/v1";
const MAX_DURATION_MS: u64 = 7 * 24 * 60 * 60 * 1_000;

pub(super) fn one_resource<T>(
    mut response: Response<T>,
    expected_type: &str,
) -> Option<Resource<T>> {
    if response.data.len() != 1 {
        return None;
    }
    let resource = response.data.pop()?;
    (resource.object_type == expected_type).then_some(resource)
}

pub(super) fn song_metadata(
    resource: Resource<SongAttributes>,
    storefront: &str,
) -> Option<CatalogTrackMetadata> {
    if resource.object_type != "songs" || !valid_numeric_id(&resource.id) {
        return None;
    }
    let attributes = resource.attributes?;
    let title = clean_text(&attributes.name?, 512)?;
    let artist = clean_text(&attributes.artist_name?, 256)?;
    let canonical_url = Url::parse(&attributes.url?)
        .ok()
        .and_then(|url| parse_catalog_url(&url).ok())
        .filter(|reference| {
            reference.provider() == CatalogProvider::AppleMusic
                && reference.kind() == CatalogItemKind::Track
                && reference.id() == resource.id
                && reference.storefront() == Some(storefront)
        })
        .map(|reference| reference.canonical_url().clone())?;
    Some(CatalogTrackMetadata {
        reference: CatalogReference::new(
            CatalogProvider::AppleMusic,
            CatalogItemKind::Track,
            resource.id,
            Some(storefront.to_owned()),
            canonical_url,
        ),
        title,
        artists: vec![artist],
        album: attributes
            .album_name
            .and_then(|value| clean_text(&value, 512)),
        duration_ms: attributes
            .duration_in_millis
            .filter(|value| *value <= MAX_DURATION_MS),
        isrc: attributes.isrc.and_then(|value| normalize_isrc(&value)),
    })
}

pub(super) fn validated_next_url(
    value: &str,
    storefront: &str,
    expected_path: &str,
) -> CatalogResult<Url> {
    if value.len() > 2_048 || value.chars().any(char::is_control) {
        return Err(CatalogError::InvalidResponse(CatalogProvider::AppleMusic));
    }
    let base = Url::parse(API_BASE).expect("constant Apple Music API base");
    let url = base
        .join(value)
        .map_err(|_| CatalogError::InvalidResponse(CatalogProvider::AppleMusic))?;
    let expected_prefix = format!("/v1/catalog/{storefront}/");
    if url.scheme() != "https"
        || url.host_str() != Some("api.music.apple.com")
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || !url.path().starts_with(&expected_prefix)
        || url.path() != expected_path
    {
        return Err(CatalogError::InvalidResponse(CatalogProvider::AppleMusic));
    }
    Ok(url)
}

pub(super) fn storefront(reference: &CatalogReference) -> CatalogResult<&str> {
    reference
        .storefront()
        .filter(|value| value.len() == 2 && value.bytes().all(|byte| byte.is_ascii_lowercase()))
        .ok_or(CatalogError::UnsupportedUrl)
}

fn valid_numeric_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= 20 && value.bytes().all(|byte| byte.is_ascii_digit())
}

pub(super) fn clean_text(value: &str, maximum_bytes: usize) -> Option<String> {
    let value = value.trim();
    (!value.is_empty() && value.len() <= maximum_bytes && !value.chars().any(char::is_control))
        .then(|| value.to_owned())
}

fn normalize_isrc(value: &str) -> Option<String> {
    let normalized = value
        .bytes()
        .filter(|byte| !matches!(byte, b'-' | b' '))
        .map(|byte| byte.to_ascii_uppercase())
        .collect::<Vec<_>>();
    (normalized.len() == 12 && normalized.iter().all(u8::is_ascii_alphanumeric))
        .then(|| String::from_utf8(normalized).expect("ASCII ISRC"))
}

#[cfg(test)]
#[path = "metadata_tests.rs"]
mod tests;
