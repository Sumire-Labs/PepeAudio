use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::{Map, Value, json};
use url::Url;

use super::*;
use crate::{
    http::{HttpResponse, tests::ScriptedTransport},
    parse_catalog_url,
};

const PLAYLIST_URL: &str = "https://open.spotify.com/playlist/37i9dQZF1DXcBWIGoYBM5M";
const FIRST_TRACK_ID: &str = "4uLU6hMCjMI75M1A2tKUQC";
const THIRD_TRACK_ID: &str = "2CQP6nWv8VvYQAYaKgy7AC";

fn reference() -> CatalogReference {
    parse_catalog_url(&Url::parse(PLAYLIST_URL).expect("URL")).expect("catalog reference")
}

fn public_document(total_count: Option<usize>) -> String {
    let playlist_uri = "spotify:playlist:37i9dQZF1DXcBWIGoYBM5M";
    let mut entities = Map::new();
    entities.insert(
        playlist_uri.to_owned(),
        json!({
            "name": "Ordered & Public",
            "uri": playlist_uri,
            "content": {
                "items": [
                    playlist_item(FIRST_TRACK_ID, "First Song", ["First Artist"], 180_000),
                    {"itemV2": {"__typename": "EpisodeResponseWrapper", "data": null}},
                    playlist_item(
                        THIRD_TRACK_ID,
                        "第三の曲",
                        ["第三アーティスト", "Guest"],
                        210_000,
                    )
                ],
                "totalCount": total_count,
                "pagingInfo": {"offset": 0}
            }
        }),
    );
    let state = json!({"entities": {"items": entities}}).to_string();
    let encoded = STANDARD.encode(state.as_bytes());
    format!("<!doctype html><script type=\"text/plain\" id=\"initialState\">{encoded}</script>")
}

fn playlist_item<const N: usize>(
    id: &str,
    title: &str,
    artists: [&str; N],
    duration_ms: u64,
) -> Value {
    json!({
        "itemV2": {
            "__typename": "TrackResponseWrapper",
            "data": {
                "__typename": "Track",
                "name": title,
                "uri": format!("spotify:track:{id}"),
                "artists": {
                    "items": artists
                        .into_iter()
                        .map(|name| json!({"profile": {"name": name}}))
                        .collect::<Vec<_>>()
                },
                "albumOfTrack": {"name": "Fixture Album"},
                "duration": {"totalMilliseconds": duration_ms}
            }
        }
    })
}

#[tokio::test]
async fn resolves_ordered_public_playlist_with_truthful_bounds() {
    let document = public_document(Some(8));
    let transport = ScriptedTransport::new([HttpResponse::new(200, document.into_bytes())]);

    let collection = resolve_public_playlist(transport.as_ref(), &reference(), 3)
        .await
        .expect("public playlist");

    assert_eq!(collection.title, "Ordered & Public");
    assert_eq!(
        collection
            .tracks
            .iter()
            .map(|track| track.title.as_str())
            .collect::<Vec<_>>(),
        ["First Song", "第三の曲"]
    );
    assert_eq!(collection.tracks[1].artists, ["第三アーティスト", "Guest"]);
    assert_eq!(collection.tracks[1].duration_ms, Some(210_000));
    assert_eq!(collection.tracks[1].album.as_deref(), Some("Fixture Album"));
    assert_eq!(collection.source_item_count, Some(8));
    assert_eq!(collection.skipped_item_count, 1);
    assert!(collection.truncated);
    assert_eq!(transport.request_urls(), [PLAYLIST_URL]);
    assert_eq!(
        transport.request_response_limit(0),
        Some(MAX_PUBLIC_PLAYLIST_RESPONSE_BYTES)
    );
    assert_eq!(transport.request_header(0, "authorization"), None);
    assert_eq!(transport.request_header(0, "cookie"), None);
}

#[tokio::test]
async fn applies_the_source_item_limit_before_filtering_invalid_entries() {
    let document = public_document(None);
    let transport = ScriptedTransport::new([HttpResponse::new(200, document.into_bytes())]);

    let collection = resolve_public_playlist(transport.as_ref(), &reference(), 2)
        .await
        .expect("bounded public playlist");

    assert_eq!(collection.tracks.len(), 1);
    assert_eq!(collection.tracks[0].reference.id(), FIRST_TRACK_ID);
    assert_eq!(collection.source_item_count, None);
    assert_eq!(collection.skipped_item_count, 1);
    assert!(collection.truncated);
}

#[tokio::test]
async fn rejects_invalid_limits_and_malformed_state_without_leaking_input() {
    let empty_transport = ScriptedTransport::new(Vec::<HttpResponse>::new());
    assert_eq!(
        resolve_public_playlist(empty_transport.as_ref(), &reference(), 0).await,
        Err(CatalogError::InvalidCollectionLimit)
    );
    assert!(empty_transport.request_urls().is_empty());

    let transport = ScriptedTransport::new([HttpResponse::new(
        200,
        b"<script id=\"initialState\" type=\"text/plain\">not-base64!</script>".to_vec(),
    )]);
    let error = resolve_public_playlist(transport.as_ref(), &reference(), 25)
        .await
        .expect_err("malformed state");
    assert_eq!(
        error,
        CatalogError::InvalidResponse(CatalogProvider::Spotify)
    );
    assert!(!error.to_string().contains(FIRST_TRACK_ID));
}

#[test]
fn rejects_an_oversized_embedded_state_before_base64_decoding() {
    let encoded = "A".repeat(MAX_INITIAL_STATE_ENCODED_BYTES + 1);
    let document = format!("<script id=\"initialState\" type=\"text/plain\">{encoded}</script>");

    assert_eq!(
        parse_public_playlist(&document, &reference(), 25),
        Err(CatalogError::ResponseTooLarge(CatalogProvider::Spotify))
    );
}
