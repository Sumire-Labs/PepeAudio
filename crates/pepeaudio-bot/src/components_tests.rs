use pepeaudio_core::{
    ChannelId, GuildId, MediaProvider, PlayerSnapshot, PlayerState, PublicMediaPage, RepeatMode,
    StateRevision, TrackProvenance, TrackSnapshot, UnixTimeMillis, Volume,
};
use uuid::Uuid;

use super::{
    HrirOption, build_ephemeral_status_panel, build_now_panel, build_status_panel,
    visible_hrir_options,
};
use crate::ComponentIdCodec;

fn snapshot() -> PlayerSnapshot {
    PlayerSnapshot {
        guild_id: GuildId::new(1).expect("guild"),
        voice_channel_id: Some(ChannelId::new(24).expect("channel")),
        revision: StateRevision::new(3),
        state: PlayerState::Disconnected,
        current_track: None,
        queued_tracks: 0,
        upcoming_tracks: Vec::new(),
        has_previous_track: false,
        volume: Volume::DEFAULT,
        repeat_mode: RepeatMode::Off,
        shuffle_enabled: false,
        hrir_preset: None,
        spatial_audio_enabled: false,
        observed_at: UnixTimeMillis::new(0),
    }
}

fn page(provider: MediaProvider, url: &str) -> PublicMediaPage {
    PublicMediaPage::new(provider, url).expect("public provider page")
}

fn now_panel_with_provenance(provenance: TrackProvenance) -> serde_json::Value {
    let mut snapshot = snapshot();
    snapshot.current_track = Some(TrackSnapshot {
        track_id: Uuid::from_u128(1),
        title: "Example".to_owned(),
        artist: Some("Artist".to_owned()),
        album: None,
        provenance: Some(provenance),
        requester_user_id: None,
        duration_ms: Some(120_000),
        position_ms: 1_000,
        seekable: true,
    });
    let codec = ComponentIdCodec::new([1; 32]).expect("codec");
    serde_json::to_value(build_now_panel(&snapshot, &codec, &[]).expect("valid panel"))
        .expect("serialize")
}

#[test]
fn now_panel_is_components_v2_only_and_neutral() {
    let codec = ComponentIdCodec::new([1; 32]).expect("codec");
    let message = build_now_panel(&snapshot(), &codec, &[]).expect("valid panel");
    let json = serde_json::to_value(message).expect("serialize");
    assert_eq!(json["flags"], 32_768);
    assert!(json.get("content").is_none());
    assert!(json.get("embeds").is_none());
    assert!(!json.to_string().contains("accent_color"));
    assert!(json.to_string().contains("<#24>"));
}

#[test]
fn now_panel_shows_origin_and_distinct_playback_pages() {
    let provenance = TrackProvenance::new(
        Some(page(
            MediaProvider::Spotify,
            "https://open.spotify.com/track/4uLU6hMCjMI75M1A2tKUQC",
        )),
        page(
            MediaProvider::YouTube,
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
        ),
    )
    .expect("provenance");

    let json = now_panel_with_provenance(provenance);
    let links = json["components"][0]["components"][2]["components"]
        .as_array()
        .expect("link row");

    assert_eq!(links.len(), 2);
    assert_eq!(links[0]["label"], "Spotifyで開く");
    assert_eq!(links[1]["label"], "YouTubeで再生");
    assert_eq!(
        links[0]["url"],
        "https://open.spotify.com/track/4uLU6hMCjMI75M1A2tKUQC"
    );
    assert_eq!(
        links[1]["url"],
        "https://www.youtube.com/watch?v=dQw4w9WgXcQ"
    );
    assert_eq!(links[0]["style"], 5);
    assert!(links[0].get("custom_id").is_none());
    assert!(!json.to_string().contains("googlevideo"));
}

#[test]
fn now_panel_deduplicates_the_same_origin_and_playback_page() {
    let youtube = page(
        MediaProvider::YouTube,
        "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
    );
    let provenance = TrackProvenance::new(Some(youtube.clone()), youtube).expect("provenance");

    let json = now_panel_with_provenance(provenance);
    let links = json["components"][0]["components"][2]["components"]
        .as_array()
        .expect("link row");

    assert_eq!(links.len(), 1);
    assert_eq!(links[0]["label"], "YouTubeで開く");
}

#[test]
fn status_panel_has_no_legacy_message_fields() {
    let message = build_status_panel("操作が完了しました。").expect("valid panel");
    let json = serde_json::to_value(message).expect("serialize");
    assert_eq!(json["flags"], 32_768);
    assert!(json.get("content").is_none());
    assert!(json.get("embeds").is_none());
    assert!(!json.to_string().contains("accent_color"));
}

#[test]
fn ephemeral_status_keeps_both_required_flags() {
    let message =
        build_ephemeral_status_panel("操作が完了しました。").expect("valid private panel");
    let json = serde_json::to_value(message).expect("serialize");
    assert_eq!(json["flags"], 32_768 | 64);
    assert!(json.get("content").is_none());
    assert!(json.get("embeds").is_none());
}

#[test]
fn selected_hrir_remains_visible_beyond_discords_option_limit() {
    let options = (0..30)
        .map(|index| HrirOption {
            id: format!("preset-{index}"),
            label: format!("Preset {index}"),
        })
        .collect::<Vec<_>>();

    let visible = visible_hrir_options(&options, Some("preset-29"));

    assert_eq!(visible.len(), 25);
    assert_eq!(
        visible.last().map(|option| option.id.as_str()),
        Some("preset-29")
    );
}
