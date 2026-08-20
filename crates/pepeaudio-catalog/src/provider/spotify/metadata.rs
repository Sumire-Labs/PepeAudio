use url::Url;

use super::wire::{NamedObject, SimplifiedTrack, Track};
use crate::{CatalogItemKind, CatalogProvider, CatalogReference, CatalogTrackMetadata};

const MAX_DURATION_MS: u64 = 7 * 24 * 60 * 60 * 1_000;

pub(super) fn track_metadata(
    track: Track,
    album_override: Option<&str>,
) -> Option<CatalogTrackMetadata> {
    if track.is_local
        || track
            .object_type
            .as_deref()
            .is_some_and(|kind| kind != "track")
    {
        return None;
    }
    let id = track.id.filter(|value| valid_spotify_id(value))?;
    let title = clean_text(&track.name, 512)?;
    let artists = artists(track.artists)?;
    let album = album_override
        .map(str::to_owned)
        .or_else(|| track.album.and_then(|album| clean_text(&album.name, 512)));
    Some(CatalogTrackMetadata {
        reference: spotify_track_reference(&id),
        title,
        artists,
        album,
        duration_ms: track.duration_ms.filter(|value| *value <= MAX_DURATION_MS),
        isrc: track
            .external_ids
            .isrc
            .and_then(|value| normalize_isrc(&value)),
    })
}

pub(super) fn simplified_track_metadata(
    track: SimplifiedTrack,
    album: &str,
) -> Option<CatalogTrackMetadata> {
    if track.is_local
        || track
            .object_type
            .as_deref()
            .is_some_and(|kind| kind != "track")
    {
        return None;
    }
    let id = track.id.filter(|value| valid_spotify_id(value))?;
    Some(CatalogTrackMetadata {
        reference: spotify_track_reference(&id),
        title: clean_text(&track.name, 512)?,
        artists: artists(track.artists)?,
        album: Some(album.to_owned()),
        duration_ms: track.duration_ms.filter(|value| *value <= MAX_DURATION_MS),
        isrc: None,
    })
}

pub(super) fn valid_market(value: &str) -> bool {
    value.len() == 2 && value.bytes().all(|byte| byte.is_ascii_uppercase())
}

fn artists(values: Vec<NamedObject>) -> Option<Vec<String>> {
    let artists = values
        .into_iter()
        .take(10)
        .filter_map(|artist| clean_text(&artist.name, 256))
        .collect::<Vec<_>>();
    (!artists.is_empty()).then_some(artists)
}

fn spotify_track_reference(id: &str) -> CatalogReference {
    CatalogReference::new(
        CatalogProvider::Spotify,
        CatalogItemKind::Track,
        id.to_owned(),
        None,
        Url::parse(&format!("https://open.spotify.com/track/{id}"))
            .expect("validated Spotify track ID"),
    )
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

pub(super) fn valid_spotify_id(value: &str) -> bool {
    value.len() == 22 && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
}
