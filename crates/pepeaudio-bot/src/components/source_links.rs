use pepeaudio_components_v2::{ButtonComponent, Component, ValidationError};
use pepeaudio_core::{MediaProvider, PlayerSnapshot, PublicMediaPage};

pub(super) fn source_links(
    snapshot: &PlayerSnapshot,
) -> Result<Option<Component>, ValidationError> {
    let Some(provenance) = snapshot
        .current_track
        .as_ref()
        .and_then(|track| track.provenance.as_ref())
    else {
        return Ok(None);
    };
    let mut buttons = Vec::with_capacity(2);
    if let Some(origin) = provenance.origin() {
        push_link(&mut buttons, origin, LinkRole::Origin);
    }
    let playback = provenance.playback();
    if provenance.origin().is_none_or(|origin| origin != playback) {
        push_link(&mut buttons, playback, LinkRole::Playback);
    }
    if buttons.is_empty() {
        return Ok(None);
    }
    Component::buttons(buttons).map(Some)
}

fn push_link(buttons: &mut Vec<ButtonComponent>, page: &PublicMediaPage, role: LinkRole) {
    if let Ok(button) = ButtonComponent::link(page.url(), link_label(page.provider(), role)) {
        buttons.push(button);
    }
}

#[derive(Clone, Copy)]
enum LinkRole {
    Origin,
    Playback,
}

fn link_label(provider: MediaProvider, role: LinkRole) -> &'static str {
    match (provider, role) {
        (MediaProvider::Spotify, _) => "Spotifyで開く",
        (MediaProvider::AppleMusic, _) => "Apple Musicで開く",
        (MediaProvider::YouTube, LinkRole::Origin) => "YouTubeで開く",
        (MediaProvider::YouTube, LinkRole::Playback) => "YouTubeで再生",
        (MediaProvider::SoundCloud, LinkRole::Origin) => "SoundCloudで開く",
        (MediaProvider::SoundCloud, LinkRole::Playback) => "SoundCloudで再生",
    }
}
