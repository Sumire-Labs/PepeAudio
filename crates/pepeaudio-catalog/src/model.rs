use std::fmt;

use url::Url;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CatalogProvider {
    Spotify,
    AppleMusic,
}

impl fmt::Display for CatalogProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Spotify => "Spotify",
            Self::AppleMusic => "Apple Music",
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CatalogItemKind {
    Track,
    Album,
    Playlist,
}

impl fmt::Display for CatalogItemKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Track => "track",
            Self::Album => "album",
            Self::Playlist => "playlist",
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogReference {
    provider: CatalogProvider,
    kind: CatalogItemKind,
    id: String,
    storefront: Option<String>,
    canonical_url: Url,
}

impl CatalogReference {
    pub(crate) fn new(
        provider: CatalogProvider,
        kind: CatalogItemKind,
        id: String,
        storefront: Option<String>,
        canonical_url: Url,
    ) -> Self {
        Self {
            provider,
            kind,
            id,
            storefront,
            canonical_url,
        }
    }

    #[must_use]
    pub const fn provider(&self) -> CatalogProvider {
        self.provider
    }

    #[must_use]
    pub const fn kind(&self) -> CatalogItemKind {
        self.kind
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn storefront(&self) -> Option<&str> {
        self.storefront.as_deref()
    }

    #[must_use]
    pub const fn canonical_url(&self) -> &Url {
        &self.canonical_url
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogTrackMetadata {
    pub reference: CatalogReference,
    pub title: String,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub duration_ms: Option<u64>,
    pub isrc: Option<String>,
}

impl CatalogTrackMetadata {
    #[must_use]
    pub fn search_request(&self) -> CatalogSearchRequest {
        CatalogSearchRequest {
            title: self.title.clone(),
            artists: self.artists.clone(),
            duration_ms: self.duration_ms,
            isrc: self.isrc.clone(),
            origin: self.reference.clone(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogSearchRequest {
    pub title: String,
    pub artists: Vec<String>,
    pub duration_ms: Option<u64>,
    pub isrc: Option<String>,
    pub origin: CatalogReference,
}

impl CatalogSearchRequest {
    /// Builds the media-resolver query without exceeding its 256-byte input
    /// boundary. The title is retained before artist names, and truncation is
    /// always performed at a UTF-8 character boundary.
    #[must_use]
    pub fn text_query(&self) -> String {
        const MAX_BYTES: usize = 256;

        let mut query = String::with_capacity(MAX_BYTES);
        append_query_component(&mut query, &self.title, MAX_BYTES);
        for artist in &self.artists {
            if query.len() == MAX_BYTES {
                break;
            }
            append_query_component(&mut query, artist, MAX_BYTES);
        }
        query
    }
}

fn append_query_component(query: &mut String, value: &str, maximum_bytes: usize) {
    let separator_bytes = usize::from(!query.is_empty());
    if query.len() + separator_bytes >= maximum_bytes {
        return;
    }
    let remaining = maximum_bytes - query.len() - separator_bytes;
    let mut component = String::new();
    for character in value.chars().filter(|character| !character.is_control()) {
        if component.len() + character.len_utf8() > remaining {
            break;
        }
        component.push(character);
    }
    let component = component.trim();
    if component.is_empty() {
        return;
    }
    if !query.is_empty() {
        query.push(' ');
    }
    query.push_str(component);
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogCollection {
    pub reference: CatalogReference,
    pub title: String,
    pub tracks: Vec<CatalogTrackMetadata>,
    pub source_item_count: Option<usize>,
    pub skipped_item_count: usize,
    pub truncated: bool,
    pub version: Option<String>,
}

impl CatalogCollection {
    #[must_use]
    pub fn search_requests(&self) -> Vec<CatalogSearchRequest> {
        self.tracks
            .iter()
            .map(CatalogTrackMetadata::search_request)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference() -> CatalogReference {
        CatalogReference::new(
            CatalogProvider::Spotify,
            CatalogItemKind::Track,
            "4uLU6hMCjMI75M1A2tKUQC".to_owned(),
            None,
            Url::parse("https://open.spotify.com/track/4uLU6hMCjMI75M1A2tKUQC")
                .expect("reference URL"),
        )
    }

    #[test]
    fn search_query_is_utf8_safe_and_within_media_limit() {
        let request = CatalogSearchRequest {
            title: "音楽".repeat(100),
            artists: vec!["アーティスト".repeat(100)],
            duration_ms: None,
            isrc: None,
            origin: reference(),
        };

        let query = request.text_query();

        assert!(query.len() <= 256);
        assert!(query.is_char_boundary(query.len()));
        assert!(!query.chars().any(char::is_control));
    }

    #[test]
    fn search_query_keeps_artists_when_the_title_leaves_room() {
        let request = CatalogSearchRequest {
            title: "Example Song".to_owned(),
            artists: vec!["Primary Artist".to_owned(), "Guest Artist".to_owned()],
            duration_ms: None,
            isrc: None,
            origin: reference(),
        };

        assert_eq!(
            request.text_query(),
            "Example Song Primary Artist Guest Artist"
        );
    }
}
