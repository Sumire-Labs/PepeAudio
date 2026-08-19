use pepeaudio_components_v2::{
    ButtonComponent, Component, Message, SelectOption, StringSelectComponent, ValidationError,
};
use serde_json::json;

#[test]
fn serializes_a_component_only_player_panel() {
    let controls = Component::buttons(vec![
        ButtonComponent::neutral("player:v7:pause", "Pause"),
        ButtonComponent::neutral("player:v7:skip", "Skip"),
        ButtonComponent::neutral("player:v7:stop", "Stop"),
        ButtonComponent::neutral("player:v7:spatial", "360° Audio"),
    ])
    .expect("valid controls");
    let presets = Component::select(
        StringSelectComponent::single(
            "player:v7:hrir",
            vec![SelectOption::new("Off", "off").selected(true)],
            Some("HRIR preset".into()),
        )
        .expect("valid select"),
    )
    .expect("valid row");
    let message = Message::new(vec![Component::container(vec![
        Component::text("# Track title\nArtist • Playing"),
        Component::separator(),
        Component::text("01:42 ━━━━━●━━━━ 03:51"),
        controls,
        presets,
    ])])
    .expect("valid message");

    let value = serde_json::to_value(message).expect("serializable");
    assert_eq!(value["flags"], json!(32768));
    assert_eq!(value["allowed_mentions"]["parse"], json!([]));
    assert_eq!(value["allowed_mentions"]["replied_user"], json!(false));
    assert_eq!(value["components"][0]["type"], json!(17));
    assert!(value["components"][0].get("accent_color").is_none());
    assert_eq!(
        value["components"][0]["components"][3]["components"][0]["style"],
        json!(2)
    );
}

#[test]
fn ephemeral_response_keeps_components_v2_flag() {
    let message = Message::ephemeral(vec![Component::text("Private error")])
        .expect("valid ephemeral response");
    let value = serde_json::to_value(message).expect("serializable");
    assert_eq!(value["flags"], json!(32768 | 64));
    assert!(value.get("content").is_none());
    assert!(value.get("embeds").is_none());
}

#[test]
fn rejects_more_than_five_buttons_in_a_row() {
    let buttons = (0..6)
        .map(|index| ButtonComponent::neutral(format!("action:{index}"), "Action"))
        .collect();
    assert_eq!(
        Component::buttons(buttons).expect_err("must reject six buttons"),
        ValidationError::InvalidButtonCount(6)
    );
}

#[test]
fn serializes_https_link_buttons_without_custom_ids() {
    let row = Component::buttons(vec![
        ButtonComponent::neutral("player:v7:pause", "Pause"),
        ButtonComponent::link(
            "https://open.spotify.com/track/4uLU6hMCjMI75M1A2tKUQC",
            "Spotify",
        )
        .expect("valid link"),
    ])
    .expect("valid controls");
    let value =
        serde_json::to_value(Message::new(vec![row]).expect("message")).expect("serializable");
    let link = &value["components"][0]["components"][1];

    assert_eq!(link["style"], json!(5));
    assert_eq!(
        link["url"],
        json!("https://open.spotify.com/track/4uLU6hMCjMI75M1A2tKUQC")
    );
    assert!(link.get("custom_id").is_none());
}

#[test]
fn rejects_unsafe_link_button_urls() {
    for url in [
        "http://open.spotify.com/track/4uLU6hMCjMI75M1A2tKUQC",
        "https://user@open.spotify.com/track/4uLU6hMCjMI75M1A2tKUQC",
        "https://open.spotify.com:444/track/4uLU6hMCjMI75M1A2tKUQC",
    ] {
        assert_eq!(
            ButtonComponent::link(url, "Source").expect_err("unsafe link must fail"),
            ValidationError::InvalidButtonUrl
        );
    }
}

#[test]
fn rejects_more_than_twenty_five_select_options() {
    let options = (0..26)
        .map(|index| SelectOption::new(format!("Preset {index}"), index.to_string()))
        .collect();
    assert_eq!(
        StringSelectComponent::single("preset", options, None).expect_err("must reject 26 options"),
        ValidationError::InvalidSelectOptionCount(26)
    );
}

#[test]
fn rejects_messages_over_the_total_component_limit() {
    let children = (0..40)
        .map(|index| Component::text(format!("line {index}")))
        .collect();
    assert_eq!(
        Message::new(vec![Component::container(children)])
            .expect_err("container plus 40 children exceeds 40 total"),
        ValidationError::TooManyComponents {
            actual: 41,
            maximum: 40,
        }
    );
}

#[test]
fn rejects_duplicate_custom_ids_across_rows() {
    let first =
        Component::buttons(vec![ButtonComponent::neutral("same", "First")]).expect("valid row");
    let second = Component::select(
        StringSelectComponent::single("same", vec![SelectOption::new("Preset", "preset")], None)
            .expect("valid select"),
    )
    .expect("valid row");
    assert_eq!(
        Message::new(vec![first, second]).expect_err("duplicate IDs must be rejected"),
        ValidationError::DuplicateCustomId("same".into())
    );
}

#[test]
fn rejects_nested_containers() {
    let nested = Component::container(vec![Component::text("nested")]);
    assert_eq!(
        Message::new(vec![Component::container(vec![nested])])
            .expect_err("containers are top-level only"),
        ValidationError::NestedContainer
    );
}
