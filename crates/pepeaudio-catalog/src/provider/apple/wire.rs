use serde::Deserialize;

#[derive(Deserialize)]
#[serde(bound(deserialize = "T: Deserialize<'de>"))]
pub(super) struct Response<T> {
    #[serde(default)]
    pub(super) data: Vec<Resource<T>>,
    pub(super) next: Option<String>,
    pub(super) meta: Option<PageMeta>,
}

#[derive(Deserialize)]
pub(super) struct PageMeta {
    pub(super) total: Option<usize>,
}

#[derive(Deserialize)]
pub(super) struct Resource<T> {
    pub(super) id: String,
    #[serde(rename = "type")]
    pub(super) object_type: String,
    pub(super) attributes: Option<T>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SongAttributes {
    pub(super) name: Option<String>,
    pub(super) artist_name: Option<String>,
    pub(super) album_name: Option<String>,
    pub(super) duration_in_millis: Option<u64>,
    pub(super) isrc: Option<String>,
    pub(super) url: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AlbumAttributes {
    pub(super) name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PlaylistAttributes {
    pub(super) name: String,
    pub(super) last_modified_date: Option<String>,
}
