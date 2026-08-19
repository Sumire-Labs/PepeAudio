use std::fmt;

use pepeaudio_core::TrackProvenance;

pub use pepeaudio_core::{MAX_TRACK_ALBUM_BYTES, MAX_TRACK_ARTIST_BYTES};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct QueueTrackMetadata {
    artist: Option<String>,
    album: Option<String>,
    provenance: Option<TrackProvenance>,
}

impl QueueTrackMetadata {
    #[must_use]
    pub fn builder() -> QueueTrackMetadataBuilder {
        QueueTrackMetadataBuilder::default()
    }

    #[must_use]
    pub fn artist(&self) -> Option<&str> {
        self.artist.as_deref()
    }

    #[must_use]
    pub fn album(&self) -> Option<&str> {
        self.album.as_deref()
    }

    #[must_use]
    pub const fn provenance(&self) -> Option<&TrackProvenance> {
        self.provenance.as_ref()
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct QueueTrackMetadataBuilder {
    metadata: QueueTrackMetadata,
}

impl QueueTrackMetadataBuilder {
    /// Sets a trimmed, bounded display artist.
    ///
    /// # Errors
    ///
    /// Returns [`TrackMetadataError`] for empty, oversized, or control-bearing
    /// external text.
    pub fn artist(mut self, value: impl AsRef<str>) -> Result<Self, TrackMetadataError> {
        self.metadata.artist = Some(validated_text(
            TrackMetadataField::Artist,
            value.as_ref(),
            MAX_TRACK_ARTIST_BYTES,
        )?);
        Ok(self)
    }

    /// Sets a trimmed, bounded display album or release name.
    ///
    /// # Errors
    ///
    /// Returns [`TrackMetadataError`] for empty, oversized, or control-bearing
    /// external text.
    pub fn album(mut self, value: impl AsRef<str>) -> Result<Self, TrackMetadataError> {
        self.metadata.album = Some(validated_text(
            TrackMetadataField::Album,
            value.as_ref(),
            MAX_TRACK_ALBUM_BYTES,
        )?);
        Ok(self)
    }

    #[must_use]
    pub fn provenance(mut self, value: TrackProvenance) -> Self {
        self.metadata.provenance = Some(value);
        self
    }

    #[must_use]
    pub fn build(self) -> QueueTrackMetadata {
        self.metadata
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrackMetadataField {
    Artist,
    Album,
}

impl fmt::Display for TrackMetadataField {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Artist => "artist",
            Self::Album => "album",
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum TrackMetadataError {
    #[error("track {0} must not be empty")]
    Empty(TrackMetadataField),
    #[error("track {0} exceeds its display limit")]
    TooLong(TrackMetadataField),
    #[error("track {0} contains a control character")]
    ControlCharacter(TrackMetadataField),
}

fn validated_text(
    field: TrackMetadataField,
    value: &str,
    maximum_bytes: usize,
) -> Result<String, TrackMetadataError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(TrackMetadataError::Empty(field));
    }
    if value.len() > maximum_bytes {
        return Err(TrackMetadataError::TooLong(field));
    }
    if value.chars().any(char::is_control) {
        return Err(TrackMetadataError::ControlCharacter(field));
    }
    Ok(value.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_trims_and_bounds_external_text() {
        let metadata = QueueTrackMetadata::builder()
            .artist("  Artist  ")
            .expect("artist")
            .album(" Album ")
            .expect("album")
            .build();
        assert_eq!(metadata.artist(), Some("Artist"));
        assert_eq!(metadata.album(), Some("Album"));

        assert_eq!(
            QueueTrackMetadata::builder().artist("A\nB").err(),
            Some(TrackMetadataError::ControlCharacter(
                TrackMetadataField::Artist
            ))
        );
        assert_eq!(
            QueueTrackMetadata::builder()
                .artist("音".repeat(MAX_TRACK_ARTIST_BYTES))
                .err(),
            Some(TrackMetadataError::TooLong(TrackMetadataField::Artist))
        );
    }
}
