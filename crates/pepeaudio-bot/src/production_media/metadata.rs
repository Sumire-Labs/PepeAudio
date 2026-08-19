use pepeaudio_catalog::{CatalogProvider, CatalogTrackMetadata};
use pepeaudio_core::{MediaProvider, PublicMediaPage, TrackProvenance};
use pepeaudio_media::{SiteProvider, SiteResolvedTrack};
use pepeaudio_player::{MAX_TRACK_ALBUM_BYTES, MAX_TRACK_ARTIST_BYTES};
use pepeaudio_player::{QueueTrackMetadata, QueueTrackMetadataBuilder};

use crate::ResolveError;

pub(super) fn from_site_track(
    resolved: &SiteResolvedTrack,
) -> Result<QueueTrackMetadata, ResolveError> {
    let provenance = TrackProvenance::new(None, playback_page(resolved)?)
        .map_err(|_| invalid_site_metadata())?;
    let mut builder = QueueTrackMetadata::builder().provenance(provenance);
    if let Some(artist) = resolved.artist.as_deref() {
        builder = add_artist(builder, artist);
    }
    if let Some(album) = resolved.album.as_deref() {
        builder = add_album(builder, album);
    }
    Ok(builder.build())
}

pub(super) fn from_catalog_track(
    catalog: &CatalogTrackMetadata,
    resolved: &SiteResolvedTrack,
) -> Result<QueueTrackMetadata, ResolveError> {
    let origin_provider = match catalog.reference.provider() {
        CatalogProvider::Spotify => MediaProvider::Spotify,
        CatalogProvider::AppleMusic => MediaProvider::AppleMusic,
    };
    let origin = PublicMediaPage::new(origin_provider, catalog.reference.canonical_url().as_str())
        .map_err(|_| invalid_site_metadata())?;
    let provenance = TrackProvenance::new(Some(origin), playback_page(resolved)?)
        .map_err(|_| invalid_site_metadata())?;
    let mut builder = QueueTrackMetadata::builder().provenance(provenance);
    if !catalog.artists.is_empty() {
        builder = add_artist(builder, &catalog.artists.join(", "));
    }
    if let Some(album) = catalog.album.as_deref() {
        builder = add_album(builder, album);
    }
    Ok(builder.build())
}

fn playback_page(resolved: &SiteResolvedTrack) -> Result<PublicMediaPage, ResolveError> {
    let provider = match resolved.provider {
        SiteProvider::YouTube => MediaProvider::YouTube,
        SiteProvider::SoundCloud => MediaProvider::SoundCloud,
    };
    PublicMediaPage::new(provider, &resolved.page_url).map_err(|_| invalid_site_metadata())
}

fn add_artist(builder: QueueTrackMetadataBuilder, artist: &str) -> QueueTrackMetadataBuilder {
    builder
        .clone()
        .artist(bounded_utf8(artist, MAX_TRACK_ARTIST_BYTES))
        .unwrap_or(builder)
}

fn add_album(builder: QueueTrackMetadataBuilder, album: &str) -> QueueTrackMetadataBuilder {
    builder
        .clone()
        .album(bounded_utf8(album, MAX_TRACK_ALBUM_BYTES))
        .unwrap_or(builder)
}

fn bounded_utf8(value: &str, maximum_bytes: usize) -> &str {
    if value.len() <= maximum_bytes {
        return value;
    }
    let mut end = maximum_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].trim_end()
}

fn invalid_site_metadata() -> ResolveError {
    ResolveError::Failed("site display metadata was invalid".into())
}

#[cfg(test)]
mod tests {
    use super::{MAX_TRACK_ARTIST_BYTES, bounded_utf8};

    #[test]
    fn multibyte_display_metadata_is_truncated_on_a_utf8_boundary() {
        let artist = "音".repeat(120);
        let bounded = bounded_utf8(&artist, MAX_TRACK_ARTIST_BYTES);
        assert!(bounded.len() <= MAX_TRACK_ARTIST_BYTES);
        assert!(!bounded.is_empty());
        assert!(artist.starts_with(bounded));
    }
}
