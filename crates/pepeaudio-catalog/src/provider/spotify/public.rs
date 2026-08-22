use std::{str, sync::Arc};

use async_trait::async_trait;
use url::Url;

use super::metadata::{MAX_DURATION_MS, clean_text, valid_spotify_id};
use crate::{
    CatalogCollection, CatalogError, CatalogItemKind, CatalogProvider, CatalogReference,
    CatalogResult, CatalogTrackMetadata,
    http::{HttpError, HttpRequest, ReqwestTransport, SharedTransport},
    provider::ProviderCatalog,
};

const PUBLIC_TRACK_BASE: &str = "https://open.spotify.com/track/";
const MAX_PUBLIC_RESPONSE_BYTES: usize = 512 * 1024;
const MAX_META_TAG_BYTES: usize = 4 * 1024;
const MAX_PUBLIC_ARTISTS: usize = 10;

pub struct SpotifyPublicCatalog {
    transport: SharedTransport,
}

impl SpotifyPublicCatalog {
    /// Creates a credential-free client for public Spotify track metadata.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogError::Transport`] when the restricted HTTP client
    /// cannot be initialized.
    pub fn new() -> CatalogResult<Self> {
        let transport = Arc::new(
            ReqwestTransport::new()
                .map_err(|_| CatalogError::Transport(CatalogProvider::Spotify))?,
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
        if !valid_spotify_id(reference.id()) {
            return Err(CatalogError::UnsupportedUrl);
        }
        let url = Url::parse(&format!("{PUBLIC_TRACK_BASE}{}", reference.id()))
            .expect("validated Spotify ID and constant origin");
        let request = HttpRequest::get(url)
            .with_header("accept", "text/html,application/xhtml+xml;q=0.9".to_owned())
            .with_header("accept-language", "en-US,en;q=0.5".to_owned())
            .with_response_limit(MAX_PUBLIC_RESPONSE_BYTES);
        let response = self
            .transport
            .execute(request)
            .await
            .map_err(map_http_error)?;
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
        let title = meta_value(document, "og:title", 512)
            .ok_or(CatalogError::InvalidResponse(CatalogProvider::Spotify))?;
        let description = meta_value(document, "og:description", 1024)
            .ok_or(CatalogError::InvalidResponse(CatalogProvider::Spotify))?;
        let artists = description
            .split_once('·')
            .and_then(|(artists, _)| public_artists(artists))
            .ok_or(CatalogError::InvalidResponse(CatalogProvider::Spotify))?;
        let duration_ms =
            meta_value(document, "music:duration", 20).and_then(|value| public_duration_ms(&value));
        let track = CatalogTrackMetadata {
            reference: reference.clone(),
            title: title.clone(),
            artists,
            album: None,
            duration_ms,
            isrc: None,
        };
        Ok(CatalogCollection {
            reference: reference.clone(),
            title,
            tracks: vec![track],
            source_item_count: Some(1),
            skipped_item_count: 0,
            truncated: false,
            version: None,
        })
    }
}

#[async_trait]
impl ProviderCatalog for SpotifyPublicCatalog {
    async fn resolve(
        &self,
        reference: &CatalogReference,
        _collection_limit: usize,
    ) -> CatalogResult<CatalogCollection> {
        if reference.provider() != CatalogProvider::Spotify {
            return Err(CatalogError::UnsupportedUrl);
        }
        if reference.kind() != CatalogItemKind::Track {
            return Err(CatalogError::PublicMetadataUnsupported {
                provider: CatalogProvider::Spotify,
                kind: reference.kind(),
            });
        }
        self.resolve_track(reference).await
    }
}

fn public_duration_ms(value: &str) -> Option<u64> {
    let duration_ms = value.trim().parse::<u64>().ok()?.checked_mul(1_000)?;
    (duration_ms > 0 && duration_ms <= MAX_DURATION_MS).then_some(duration_ms)
}

fn public_artists(value: &str) -> Option<Vec<String>> {
    let artists = value
        .split(',')
        .take(MAX_PUBLIC_ARTISTS)
        .filter_map(|artist| clean_text(artist, 256))
        .collect::<Vec<_>>();
    (!artists.is_empty()).then_some(artists)
}

fn meta_value(document: &str, property: &str, maximum_bytes: usize) -> Option<String> {
    let bytes = document.as_bytes();
    let mut cursor = 0;
    while let Some(start) = find_ascii_case_insensitive(bytes, b"<meta", cursor) {
        let end = find_tag_end(bytes, start)?;
        let tag = &document[start..=end];
        let matches_property = attribute(tag, "property")
            .or_else(|| attribute(tag, "name"))
            .is_some_and(|value| value.eq_ignore_ascii_case(property));
        if matches_property && let Some(content) = attribute(tag, "content") {
            let decoded = decode_html_entities(content, maximum_bytes)?;
            return clean_text(&decoded, maximum_bytes);
        }
        cursor = end + 1;
    }
    None
}

fn find_ascii_case_insensitive(haystack: &[u8], needle: &[u8], start: usize) -> Option<usize> {
    haystack
        .get(start..)?
        .windows(needle.len())
        .position(|window| window.eq_ignore_ascii_case(needle))
        .map(|offset| start + offset)
}

fn find_tag_end(bytes: &[u8], start: usize) -> Option<usize> {
    let boundary = bytes.len().min(start.saturating_add(MAX_META_TAG_BYTES));
    let mut quote = None;
    for (offset, byte) in bytes.get(start..boundary)?.iter().copied().enumerate() {
        match (quote, byte) {
            (None, b'\'' | b'"') => quote = Some(byte),
            (Some(open), close) if open == close => quote = None,
            (None, b'>') => return Some(start + offset),
            _ => {}
        }
    }
    None
}

fn attribute<'a>(tag: &'a str, wanted: &str) -> Option<&'a str> {
    let bytes = tag.as_bytes();
    let mut cursor = b"<meta".len();
    while cursor < bytes.len() {
        while bytes
            .get(cursor)
            .is_some_and(|byte| byte.is_ascii_whitespace() || *byte == b'/')
        {
            cursor += 1;
        }
        if bytes.get(cursor).is_none_or(|byte| *byte == b'>') {
            break;
        }
        let name_start = cursor;
        while bytes
            .get(cursor)
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || matches!(*byte, b'-' | b'_' | b':'))
        {
            cursor += 1;
        }
        if cursor == name_start {
            cursor += 1;
            continue;
        }
        let name = &tag[name_start..cursor];
        while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
            cursor += 1;
        }
        if bytes.get(cursor) != Some(&b'=') {
            continue;
        }
        cursor += 1;
        while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
            cursor += 1;
        }
        let quote = *bytes.get(cursor)?;
        if !matches!(quote, b'\'' | b'"') {
            return None;
        }
        cursor += 1;
        let value_start = cursor;
        while bytes.get(cursor).is_some_and(|byte| *byte != quote) {
            cursor += 1;
        }
        let value = tag.get(value_start..cursor)?;
        cursor += 1;
        if name.eq_ignore_ascii_case(wanted) {
            return Some(value);
        }
    }
    None
}

fn decode_html_entities(value: &str, maximum_bytes: usize) -> Option<String> {
    let mut decoded = String::with_capacity(value.len().min(maximum_bytes));
    let mut cursor = 0;
    while cursor < value.len() {
        let entity_search_end = value.len().min(cursor.saturating_add(13));
        if value.as_bytes()[cursor] == b'&'
            && let Some(relative_end) = value.as_bytes()[cursor + 1..entity_search_end]
                .iter()
                .position(|byte| *byte == b';')
        {
            let end = cursor + 1 + relative_end;
            if let Some(character) = html_entity(&value[cursor + 1..end]) {
                push_bounded(&mut decoded, character, maximum_bytes)?;
                cursor = end + 1;
                continue;
            }
        }
        let character = value[cursor..].chars().next()?;
        push_bounded(&mut decoded, character, maximum_bytes)?;
        cursor += character.len_utf8();
    }
    Some(decoded)
}

fn html_entity(entity: &str) -> Option<char> {
    match entity {
        "amp" => Some('&'),
        "apos" | "#39" => Some('\''),
        "quot" => Some('"'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "nbsp" => Some(' '),
        value if value.starts_with("#x") || value.starts_with("#X") => {
            u32::from_str_radix(&value[2..], 16)
                .ok()
                .and_then(char::from_u32)
        }
        value if value.starts_with('#') => value[1..].parse().ok().and_then(char::from_u32),
        _ => None,
    }
}

fn push_bounded(output: &mut String, character: char, maximum_bytes: usize) -> Option<()> {
    (output.len() + character.len_utf8() <= maximum_bytes).then(|| output.push(character))
}

fn map_http_error(error: HttpError) -> CatalogError {
    match error {
        HttpError::Transport => CatalogError::Transport(CatalogProvider::Spotify),
        HttpError::ResponseTooLarge => CatalogError::ResponseTooLarge(CatalogProvider::Spotify),
    }
}

#[cfg(test)]
#[path = "public_tests.rs"]
mod tests;
