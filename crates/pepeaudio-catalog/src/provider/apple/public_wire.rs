use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LookupResponse {
    pub(super) result_count: usize,
    pub(super) results: Vec<LookupResult>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LookupResult {
    pub(super) wrapper_type: Option<String>,
    pub(super) kind: Option<String>,
    pub(super) collection_type: Option<String>,
    pub(super) track_id: Option<u64>,
    pub(super) collection_id: Option<u64>,
    pub(super) track_name: Option<String>,
    pub(super) artist_name: Option<String>,
    pub(super) collection_name: Option<String>,
    pub(super) track_time_millis: Option<u64>,
    pub(super) track_count: Option<usize>,
}
