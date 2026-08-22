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
fn now_panel_is_components_v2_only() {
    let codec = ComponentIdCodec::new([1; 32]).expect("codec");
    let message = build_now_panel(&snapshot(), &codec, &[]).expect("valid panel");
    let json = serde_json::to_value(message).expect("serialize");
    assert_eq!(json["flags"], 32_768);
    assert!(json.get("content").is_none());
    assert!(json.get("embeds").is_none());
    assert!(!json.to_string().contains("accent_color"));
    assert!(!json.to_string().contains("<#24>"));
    assert!(!json.to_string().contains("状態:"));
    assert!(!json.to_string().contains("ボイス:"));
}

#[test]
fn now_panel_embeds_the_origin_in_the_title_without_link_buttons() {
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
    let serialized = json.to_string();
    assert!(!serialized.contains("Spotifyで開く"));
    assert!(!serialized.contains("YouTubeで再生"));
    assert!(
        serialized.contains("## [Example](https://open.spotify.com/track/4uLU6hMCjMI75M1A2tKUQC)")
    );
    assert!(!serialized.contains("youtube.com"));
    assert!(serialized.contains("https://i.ytimg.com/vi/dQw4w9WgXcQ/hqdefault.jpg"));
    assert!(!serialized.contains("\"style\":5"));
}

#[test]
fn now_panel_shows_elapsed_and_total_time_around_the_progress_bar() {
    let provenance = TrackProvenance::new(
        None,
        page(
            MediaProvider::YouTube,
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
        ),
    )
    .expect("provenance");
    let serialized = now_panel_with_provenance(provenance).to_string();

    assert!(serialized.contains('●'));
    assert!(serialized.contains('─'));
    assert!(serialized.contains("0:01"));
    assert!(serialized.contains("2:00"));
    assert!(!serialized.contains("音量:"));
    assert!(!serialized.contains("キュー:"));
}

#[test]
fn transport_and_toggle_buttons_use_symbols_and_state_colors() {
    let mut active = snapshot();
    active.state = PlayerState::Playing;
    active.repeat_mode = RepeatMode::Track;
    active.shuffle_enabled = true;
    active.spatial_audio_enabled = true;
    active.current_track = Some(TrackSnapshot {
        track_id: Uuid::from_u128(1),
        title: "Example".to_owned(),
        artist: None,
        album: None,
        provenance: None,
        requester_user_id: None,
        duration_ms: Some(120_000),
        position_ms: 1_000,
        seekable: true,
    });
    let codec = ComponentIdCodec::new([1; 32]).expect("codec");
    let json = serde_json::to_value(build_now_panel(&active, &codec, &[]).expect("valid panel"))
        .expect("serialize");
    let rows = json["components"][0]["components"]
        .as_array()
        .expect("container children");
    let transport = rows[2]["components"].as_array().expect("transport row");
    let modes = rows[3]["components"].as_array().expect("mode row");

    assert_eq!(
        transport
            .iter()
            .map(|button| button["label"].as_str().expect("label"))
            .collect::<Vec<_>>(),
        vec!["⏮", "⏸", "⏭", "⏹"]
    );
    assert_eq!(transport[1]["style"], 1);
    assert_eq!(transport[3]["style"], 4);
    assert!(modes.iter().all(|button| button["style"] == 1));
}

#[test]
fn volume_selector_uses_five_percent_steps() {
    let codec = ComponentIdCodec::new([1; 32]).expect("codec");
    let json = serde_json::to_value(build_now_panel(&snapshot(), &codec, &[]).expect("panel"))
        .expect("serialize");
    let options = json["components"][0]["components"][4]["components"][0]["options"]
        .as_array()
        .expect("volume options");

    assert_eq!(options.len(), 21);
    assert_eq!(options.first().expect("first")["value"], "0");
    assert_eq!(options.last().expect("last")["value"], "100");
    assert_eq!(options[2]["value"], "10");
    assert_eq!(options[2]["default"], true);
    assert!(options.iter().all(|option| {
        option["value"]
            .as_str()
            .and_then(|value| value.parse::<u16>().ok())
            .is_some_and(|value| value % 5 == 0)
    }));
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
            description: None,
        })
        .collect::<Vec<_>>();

    let visible = visible_hrir_options(&options, Some("preset-29"));

    assert_eq!(visible.len(), 24);
    assert_eq!(
        visible.last().map(|option| option.id.as_str()),
        Some("preset-29")
    );
}

#[test]
fn hrir_selector_includes_off_and_reflects_spatial_state() {
    let codec = ComponentIdCodec::new([1; 32]).expect("codec");
    let options = [HrirOption {
        id: "dht".into(),
        label: "Aura Cinema 4.1".into(),
        description: None,
    }];

    let off =
        serde_json::to_value(build_now_panel(&snapshot(), &codec, &options).expect("off panel"))
            .expect("serialize");
    let off_options = off["components"][0]["components"][5]["components"][0]["options"]
        .as_array()
        .expect("HRIR options");
    assert_eq!(off_options[0]["label"], "オフ");
    assert_eq!(off_options[0]["value"], super::HRIR_OFF_VALUE);
    assert_eq!(off_options[0]["default"], true);

    let mut enabled = snapshot();
    enabled.spatial_audio_enabled = true;
    enabled.hrir_preset = Some(pepeaudio_core::HrirPresetId::new("dht").expect("preset"));
    let on =
        serde_json::to_value(build_now_panel(&enabled, &codec, &options).expect("enabled panel"))
            .expect("serialize");
    let on_options = on["components"][0]["components"][5]["components"][0]["options"]
        .as_array()
        .expect("HRIR options");
    assert_eq!(on_options[0]["default"], false);
    assert_eq!(on_options[1]["default"], true);
}

#[test]
fn hrir_selector_exposes_a_bounded_secondary_description() {
    let codec = ComponentIdCodec::new([1; 32]).expect("codec");
    let description = "A detailed spatial preset description ".repeat(4);
    let options = [HrirOption {
        id: "dht".into(),
        label: "Aura Cinema 4.1".into(),
        description: Some(description),
    }];
    let message = build_now_panel(&snapshot(), &codec, &options).expect("valid panel");
    let json = serde_json::to_value(message).expect("serialize");
    let serialized = json.to_string();

    assert!(serialized.contains("Aura Cinema 4.1"));
    assert!(serialized.contains("A detailed spatial preset description"));
    assert!(!serialized.contains(&"description ".repeat(4)));
}
