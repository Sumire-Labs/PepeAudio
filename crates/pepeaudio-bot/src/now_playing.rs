use pepeaudio_components_v2::{Component, SectionComponent, TextDisplayComponent, ValidationError};
use pepeaudio_core::{MediaProvider, PlayerSnapshot, TrackSnapshot};
use url::Url;

use crate::display_text::escape_discord_markdown;

const PROGRESS_SEGMENTS: usize = 18;

pub(crate) fn status_components(
    snapshot: &PlayerSnapshot,
) -> Result<Vec<Component>, ValidationError> {
    let title = title(snapshot);
    let thumbnail = snapshot.current_track.as_ref().and_then(youtube_thumbnail);
    let title = match thumbnail {
        Some(thumbnail) => SectionComponent::with_thumbnail(
            vec![TextDisplayComponent::new(title)],
            thumbnail,
            Some("再生中の曲のサムネイル".to_owned()),
        )
        .map(Component::Section)?,
        None => Component::text(title),
    };

    Ok(vec![title, Component::text(progress(snapshot))])
}

fn title(snapshot: &PlayerSnapshot) -> String {
    snapshot.current_track.as_ref().map_or_else(
        || "## 再生中の曲はありません".to_owned(),
        |track| {
            let title = escape_discord_markdown(&track.title);
            track.provenance.as_ref().map_or_else(
                || format!("## {title}"),
                |provenance| {
                    let page = provenance.origin().unwrap_or_else(|| provenance.playback());
                    format!("## [{title}]({})", markdown_link_destination(page.url()))
                },
            )
        },
    )
}

fn youtube_thumbnail(track: &TrackSnapshot) -> Option<String> {
    let playback = track.provenance.as_ref()?.playback();
    if playback.provider() != MediaProvider::YouTube {
        return None;
    }
    let url = Url::parse(playback.url()).ok()?;
    let identifier = match url.host_str()? {
        "youtu.be" => url.path_segments()?.next()?.to_owned(),
        "youtube.com" | "www.youtube.com" => url
            .query_pairs()
            .find_map(|(name, value)| (name == "v").then(|| value.into_owned()))?,
        _ => return None,
    };
    if identifier.len() != 11
        || !identifier
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return None;
    }
    Some(format!("https://i.ytimg.com/vi/{identifier}/hqdefault.jpg"))
}

fn markdown_link_destination(value: &str) -> String {
    value.replace('(', "%28").replace(')', "%29")
}

fn progress(snapshot: &PlayerSnapshot) -> String {
    let Some(track) = snapshot.current_track.as_ref() else {
        return format!("`0:00` `{}` `0:00`", "─".repeat(PROGRESS_SEGMENTS));
    };
    let elapsed = format_millis(track.position_ms);
    let Some(duration) = track.duration_ms.filter(|duration| *duration > 0) else {
        return format!("`{elapsed}` `{}` `LIVE`", "━".repeat(PROGRESS_SEGMENTS));
    };
    let maximum_index = PROGRESS_SEGMENTS - 1;
    let marker = u128::from(track.position_ms.min(duration))
        .saturating_mul(u128::try_from(maximum_index).unwrap_or(u128::MAX))
        / u128::from(duration);
    let marker = usize::try_from(marker).unwrap_or(maximum_index);
    let bar = format!(
        "{}●{}",
        "━".repeat(marker),
        "─".repeat(maximum_index.saturating_sub(marker))
    );
    format!("`{elapsed}` `{bar}` `{}`", format_millis(duration))
}

fn format_millis(milliseconds: u64) -> String {
    let seconds = milliseconds / 1_000;
    let minutes = seconds / 60;
    if minutes < 60 {
        return format!("{minutes}:{:02}", seconds % 60);
    }
    format!("{}:{:02}:{:02}", minutes / 60, minutes % 60, seconds % 60)
}

#[cfg(test)]
mod tests {
    use super::format_millis;

    #[test]
    fn formats_short_and_long_positions() {
        assert_eq!(format_millis(8_900), "0:08");
        assert_eq!(format_millis(273_000), "4:33");
        assert_eq!(format_millis(3_723_000), "1:02:03");
    }
}
