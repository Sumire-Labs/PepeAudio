use serde::Deserialize;

#[derive(Deserialize)]
pub(super) struct TokenResponse {
    pub(super) access_token: String,
    pub(super) token_type: String,
    pub(super) expires_in: u64,
}

#[derive(Deserialize)]
pub(super) struct NamedObject {
    pub(super) name: String,
}

#[derive(Default, Deserialize)]
pub(super) struct ExternalIds {
    pub(super) isrc: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct Track {
    pub(super) id: Option<String>,
    pub(super) name: String,
    #[serde(default)]
    pub(super) artists: Vec<NamedObject>,
    pub(super) album: Option<NamedObject>,
    pub(super) duration_ms: Option<u64>,
    #[serde(default)]
    pub(super) external_ids: ExternalIds,
    #[serde(rename = "type")]
    pub(super) object_type: Option<String>,
    #[serde(default)]
    pub(super) is_local: bool,
}

#[derive(Deserialize)]
pub(super) struct SimplifiedTrack {
    pub(super) id: Option<String>,
    pub(super) name: String,
    #[serde(default)]
    pub(super) artists: Vec<NamedObject>,
    pub(super) duration_ms: Option<u64>,
    #[serde(rename = "type")]
    pub(super) object_type: Option<String>,
    #[serde(default)]
    pub(super) is_local: bool,
}

#[derive(Deserialize)]
#[serde(bound(deserialize = "T: Deserialize<'de>"))]
pub(super) struct Page<T> {
    #[serde(default)]
    pub(super) items: Vec<T>,
    pub(super) total: Option<usize>,
    pub(super) next: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct Album {
    pub(super) name: String,
    pub(super) tracks: Page<SimplifiedTrack>,
}
