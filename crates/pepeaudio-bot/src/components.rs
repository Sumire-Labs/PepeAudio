use crate::{ComponentAction, ComponentIdCodec, display_text::escape_discord_markdown};
use pepeaudio_components_v2::{
    ButtonComponent, Component, Message, SelectOption, StringSelectComponent, ValidationError,
};
use pepeaudio_core::{PlayerSnapshot, PlayerState, RepeatMode};

mod source_links;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HrirOption {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
}

/// Builds a component-only `/now` response without content or embeds.
///
/// # Errors
///
/// Returns when metadata cannot fit Discord's Components V2 constraints.
pub fn build_now_panel(
    snapshot: &PlayerSnapshot,
    component_ids: &ComponentIdCodec,
    hrir_options: &[HrirOption],
) -> Result<Message, ValidationError> {
    let action = |action| component_ids.encode(action, snapshot.guild_id, snapshot.revision);
    let mut children = status_components(snapshot);
    if let Some(links) = source_links::source_links(snapshot)? {
        children.push(links);
    }
    children.push(playback_controls(snapshot, &action)?);
    children.push(mode_controls(snapshot, &action)?);
    children.push(volume_selector(snapshot, &action)?);
    if let Some(selector) = hrir_selector(snapshot, hrir_options, &action)? {
        children.push(selector);
    }
    if hrir_options.len() > 25 {
        children.push(Component::text(
            "Discordでは25件まで表示しています。すべてのHRIRプリセットはWebダッシュボードで選べます。",
        ));
    }
    Message::new(vec![Component::container(children)])
}

/// # Errors
///
/// Returns when the text cannot form a valid Components V2 message.
pub fn build_status_panel(text: impl Into<String>) -> Result<Message, ValidationError> {
    Message::new(vec![Component::container(vec![Component::text(text)])])
}

/// # Errors
///
/// Returns when the text cannot form a valid Components V2 message.
pub fn build_ephemeral_status_panel(text: impl Into<String>) -> Result<Message, ValidationError> {
    Message::ephemeral(vec![Component::container(vec![Component::text(text)])])
}

fn status_components(snapshot: &PlayerSnapshot) -> Vec<Component> {
    let title = snapshot
        .current_track
        .as_ref()
        .map_or("再生中の曲はありません", |track| {
            track.title.as_str()
        });
    let voice_channel = snapshot.voice_channel_id.map_or_else(
        || "未接続".to_owned(),
        |channel_id| format!("<#{channel_id}>"),
    );
    let status = format!(
        "## {}\n状態: `{}` · ボイス: {} · 音量: `{}%` · キュー: `{}曲`",
        escape_discord_markdown(title),
        state_label(snapshot.state),
        voice_channel,
        snapshot.volume.percent(),
        snapshot.queued_tracks
    );
    let progress = snapshot.current_track.as_ref().map_or_else(
        || "`--:-- / --:--`".to_owned(),
        |track| {
            format!(
                "`{} / {}`",
                format_duration(track.position_ms),
                track
                    .duration_ms
                    .map_or_else(|| "LIVE".to_owned(), format_duration)
            )
        },
    );

    vec![Component::text(status), Component::text(progress)]
}

fn playback_controls(
    snapshot: &PlayerSnapshot,
    action: &impl Fn(ComponentAction) -> String,
) -> Result<Component, ValidationError> {
    Component::buttons(vec![
        ButtonComponent::neutral(action(ComponentAction::Previous), "前へ")
            .disabled(!snapshot.has_previous_track),
        ButtonComponent::neutral(
            action(ComponentAction::PlayPause),
            if snapshot.state == PlayerState::Playing {
                "一時停止"
            } else {
                "再生"
            },
        )
        .disabled(snapshot.current_track.is_none()),
        ButtonComponent::neutral(action(ComponentAction::Skip), "スキップ")
            .disabled(snapshot.current_track.is_none()),
        ButtonComponent::neutral(action(ComponentAction::Stop), "停止")
            .disabled(snapshot.current_track.is_none() && snapshot.queued_tracks == 0),
    ])
}

fn mode_controls(
    snapshot: &PlayerSnapshot,
    action: &impl Fn(ComponentAction) -> String,
) -> Result<Component, ValidationError> {
    Component::buttons(vec![
        ButtonComponent::neutral(
            action(ComponentAction::Repeat),
            format!("リピート: {}", repeat_label(snapshot.repeat_mode)),
        ),
        ButtonComponent::neutral(
            action(ComponentAction::Shuffle),
            if snapshot.shuffle_enabled {
                "シャッフル: 有効"
            } else {
                "シャッフル: 無効"
            },
        ),
        ButtonComponent::neutral(
            action(ComponentAction::Spatial),
            if snapshot.spatial_audio_enabled {
                "360° Audio: 有効"
            } else {
                "360° Audio: 無効"
            },
        ),
    ])
}

fn volume_selector(
    snapshot: &PlayerSnapshot,
    action: &impl Fn(ComponentAction) -> String,
) -> Result<Component, ValidationError> {
    let current = u16::from(snapshot.volume.percent());
    let mut values = (0..=10).map(|step| step * 10).collect::<Vec<_>>();
    if !values.contains(&current) {
        values.push(current);
        values.sort_unstable();
    }
    let volumes = values
        .into_iter()
        .map(|value| {
            SelectOption::new(format!("{value}%"), value.to_string()).selected(value == current)
        })
        .collect();
    Component::select(StringSelectComponent::single(
        action(ComponentAction::Volume),
        volumes,
        Some("音量".into()),
    )?)
}

fn hrir_selector(
    snapshot: &PlayerSnapshot,
    hrir_options: &[HrirOption],
    action: &impl Fn(ComponentAction) -> String,
) -> Result<Option<Component>, ValidationError> {
    if hrir_options.is_empty() {
        return Ok(None);
    }
    let selected = snapshot
        .hrir_preset
        .as_ref()
        .map(pepeaudio_core::HrirPresetId::as_str);
    let options = visible_hrir_options(hrir_options, selected)
        .into_iter()
        .map(|option| {
            let item = SelectOption::new(option.label.clone(), option.id.clone());
            let item = option
                .description
                .as_deref()
                .map_or(item.clone(), |description| {
                    item.description(discord_option_description(description))
                });
            item.selected(selected == Some(option.id.as_str()))
        })
        .collect();
    Component::select(StringSelectComponent::single(
        action(ComponentAction::Hrir),
        options,
        Some("HRIRプリセット".into()),
    )?)
    .map(Some)
}

fn discord_option_description(value: &str) -> String {
    const MAXIMUM: usize = 100;
    if value.chars().count() <= MAXIMUM {
        return value.to_owned();
    }
    let mut shortened = value.chars().take(MAXIMUM - 1).collect::<String>();
    if let Some(word_boundary) = shortened.rfind(char::is_whitespace) {
        shortened.truncate(word_boundary);
    }
    shortened.push('…');
    shortened
}

fn visible_hrir_options<'a>(
    options: &'a [HrirOption],
    selected: Option<&str>,
) -> Vec<&'a HrirOption> {
    let mut visible = options.iter().take(25).collect::<Vec<_>>();
    let Some(selected) = selected else {
        return visible;
    };
    if visible.iter().any(|option| option.id == selected) {
        return visible;
    }
    if let Some(selected_option) = options.iter().skip(25).find(|option| option.id == selected)
        && let Some(last) = visible.last_mut()
    {
        *last = selected_option;
    }
    visible
}

fn state_label(state: PlayerState) -> &'static str {
    match state {
        PlayerState::Disconnected => "切断",
        PlayerState::IdleConnected => "待機",
        PlayerState::Loading => "読込中",
        PlayerState::Playing => "再生中",
        PlayerState::Paused => "一時停止",
    }
}

fn repeat_label(mode: RepeatMode) -> &'static str {
    match mode {
        RepeatMode::Off => "無効",
        RepeatMode::Track => "曲",
        RepeatMode::Queue => "キュー",
    }
}

fn format_duration(milliseconds: u64) -> String {
    let seconds = milliseconds / 1_000;
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

#[cfg(test)]
#[path = "components_tests.rs"]
mod tests;
