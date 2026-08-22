use std::{collections::HashMap, str};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::Deserialize;
use serde_json::Value;

use super::{
    metadata::{MAX_DURATION_MS, clean_text, spotify_track_reference, valid_spotify_id},
    public::{attribute, find_ascii_case_insensitive, find_tag_end, map_http_error},
};
use crate::{
    CatalogCollection, CatalogError, CatalogProvider, CatalogReference, CatalogResult,
    CatalogTrackMetadata,
    http::{HttpRequest, HttpTransport},
};

pub(super) const MAX_PUBLIC_PLAYLIST_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const MAX_INITIAL_STATE_ENCODED_BYTES: usize = 1024 * 1024;
const MAX_INITIAL_STATE_BYTES: usize = 768 * 1024;
const MAX_COLLECTION_ITEMS: usize = 100;
const MAX_ARTISTS: usize = 10;

pub(super) async fn resolve_public_playlist(
    transport: &dyn HttpTransport,
    reference: &CatalogReference,
    collection_limit: usize,
) -> CatalogResult<CatalogCollection> {
    if !(1..=MAX_COLLECTION_ITEMS).contains(&collection_limit) {
        return Err(CatalogError::InvalidCollectionLimit);
    }
    let request = HttpRequest::get(reference.canonical_url().clone())
        .with_header("accept", "text/html,application/xhtml+xml;q=0.9".to_owned())
        .with_header("accept-language", "en-US,en;q=0.5".to_owned())
        .with_response_limit(MAX_PUBLIC_PLAYLIST_RESPONSE_BYTES);
    let response = transport.execute(request).await.map_err(map_http_error)?;
    match response.status {
        200 => {}
        401 | 403 => return Err(CatalogError::AccessDenied(CatalogProvider::Spotify)),
        404 => return Err(CatalogError::NotFound(CatalogProvider::Spotify)),
        429 => {
            return Err(CatalogError::RateLimited {
                provider: CatalogProvider::Spotify,
                retry_after_seconds: response.retry_after_seconds,
            });
        }
        _ => return Err(CatalogError::Transport(CatalogProvider::Spotify)),
    }
    let document = str::from_utf8(&response.body)
        .map_err(|_| CatalogError::InvalidResponse(CatalogProvider::Spotify))?;
    parse_public_playlist(document, reference, collection_limit)
}

fn parse_public_playlist(
    document: &str,
    reference: &CatalogReference,
    collection_limit: usize,
) -> CatalogResult<CatalogCollection> {
    let encoded =
        initial_state(document).ok_or(CatalogError::InvalidResponse(CatalogProvider::Spotify))?;
    if encoded.len() > MAX_INITIAL_STATE_ENCODED_BYTES {
        return Err(CatalogError::ResponseTooLarge(CatalogProvider::Spotify));
    }
    let decoded = STANDARD
        .decode(encoded)
        .map_err(|_| CatalogError::InvalidResponse(CatalogProvider::Spotify))?;
    if decoded.len() > MAX_INITIAL_STATE_BYTES {
        return Err(CatalogError::ResponseTooLarge(CatalogProvider::Spotify));
    }
    let mut state: PublicState = serde_json::from_slice(&decoded)
        .map_err(|_| CatalogError::InvalidResponse(CatalogProvider::Spotify))?;
    let entity_key = format!("spotify:playlist:{}", reference.id());
    let entity = state
        .entities
        .items
        .remove(&entity_key)
        .ok_or(CatalogError::InvalidResponse(CatalogProvider::Spotify))?;
    let playlist: PublicPlaylist = serde_json::from_value(entity)
        .map_err(|_| CatalogError::InvalidResponse(CatalogProvider::Spotify))?;
    if playlist.uri != entity_key {
        return Err(CatalogError::InvalidResponse(CatalogProvider::Spotify));
    }
    let title = clean_text(&playlist.name, 512)
        .ok_or(CatalogError::InvalidResponse(CatalogProvider::Spotify))?;
    let available_items = playlist.content.items.len();
    if playlist
        .content
        .total_count
        .is_some_and(|count| count < available_items)
    {
        return Err(CatalogError::InvalidResponse(CatalogProvider::Spotify));
    }
    let processed_items = available_items.min(collection_limit);
    let mut skipped_item_count = 0;
    let tracks = playlist
        .content
        .items
        .into_iter()
        .take(processed_items)
        .filter_map(|item| {
            if let Some(track) = playlist_track(item) {
                Some(track)
            } else {
                skipped_item_count += 1;
                None
            }
        })
        .collect();
    let source_item_count = playlist.content.total_count;
    let truncated = available_items > processed_items
        || source_item_count.is_some_and(|count| count > processed_items);
    Ok(CatalogCollection {
        reference: reference.clone(),
        title,
        tracks,
        source_item_count,
        skipped_item_count,
        truncated,
        version: None,
    })
}

fn initial_state(document: &str) -> Option<&str> {
    let bytes = document.as_bytes();
    let mut cursor = 0;
    while let Some(start) = find_ascii_case_insensitive(bytes, b"<script", cursor) {
        let tag_end = find_tag_end(bytes, start)?;
        let tag = document.get(start..=tag_end)?;
        let is_initial_state = attribute(tag, "id") == Some("initialState")
            && attribute(tag, "type").is_some_and(|value| value.eq_ignore_ascii_case("text/plain"));
        if is_initial_state {
            let content_start = tag_end + 1;
            let content_end = find_ascii_case_insensitive(bytes, b"</script>", content_start)?;
            return document.get(content_start..content_end).map(str::trim);
        }
        cursor = tag_end + 1;
    }
    None
}

fn playlist_track(item: PublicPlaylistItem) -> Option<CatalogTrackMetadata> {
    let wrapper = item.item_v2?;
    if wrapper.type_name.as_deref() != Some("TrackResponseWrapper") {
        return None;
    }
    let track: PublicTrack = serde_json::from_value(wrapper.data?).ok()?;
    if track.type_name.as_deref() != Some("Track") {
        return None;
    }
    let id = track.uri.strip_prefix("spotify:track:")?;
    if !valid_spotify_id(id) {
        return None;
    }
    let artists = track
        .artists
        .items
        .into_iter()
        .take(MAX_ARTISTS)
        .filter_map(|artist| clean_text(&artist.profile.name, 256))
        .collect::<Vec<_>>();
    if artists.is_empty() {
        return None;
    }
    Some(CatalogTrackMetadata {
        reference: spotify_track_reference(id),
        title: clean_text(&track.name, 512)?,
        artists,
        album: track
            .album_of_track
            .and_then(|album| clean_text(&album.name, 512)),
        duration_ms: track.duration.and_then(|duration| {
            (duration.total_milliseconds > 0 && duration.total_milliseconds <= MAX_DURATION_MS)
                .then_some(duration.total_milliseconds)
        }),
        isrc: None,
    })
}

#[derive(Deserialize)]
struct PublicState {
    entities: PublicEntities,
}

#[derive(Deserialize)]
struct PublicEntities {
    items: HashMap<String, Value>,
}

#[derive(Deserialize)]
struct PublicPlaylist {
    name: String,
    uri: String,
    content: PublicPlaylistContent,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PublicPlaylistContent {
    #[serde(default)]
    items: Vec<PublicPlaylistItem>,
    total_count: Option<usize>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PublicPlaylistItem {
    item_v2: Option<PublicTrackWrapper>,
}

#[derive(Deserialize)]
struct PublicTrackWrapper {
    #[serde(rename = "__typename")]
    type_name: Option<String>,
    data: Option<Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PublicTrack {
    #[serde(rename = "__typename")]
    type_name: Option<String>,
    name: String,
    uri: String,
    artists: PublicArtists,
    album_of_track: Option<PublicAlbum>,
    duration: Option<PublicDuration>,
}

#[derive(Deserialize)]
struct PublicArtists {
    #[serde(default)]
    items: Vec<PublicArtist>,
}

#[derive(Deserialize)]
struct PublicArtist {
    profile: PublicArtistProfile,
}

#[derive(Deserialize)]
struct PublicArtistProfile {
    name: String,
}

#[derive(Deserialize)]
struct PublicAlbum {
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PublicDuration {
    total_milliseconds: u64,
}

#[cfg(test)]
#[path = "public_playlist_tests.rs"]
mod tests;
