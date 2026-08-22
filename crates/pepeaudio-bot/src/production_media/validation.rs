use std::time::Duration;

use pepeaudio_media::{FetchError, IngestError, ProcessError, StoreError};
use url::Url;

use crate::ResolveError;

pub(super) fn supported_format(format_name: Option<&str>) -> bool {
    const ALLOWED: &[&str] = &[
        "aac", "aiff", "asf", "flac", "matroska", "mov", "mp3", "ogg", "opus", "wav",
    ];
    format_name.is_some_and(|formats| {
        formats
            .split(',')
            .map(str::trim)
            .any(|format| ALLOWED.contains(&format))
    })
}

pub(super) fn duration_ms(seconds: Option<f64>) -> Result<Option<u64>, ResolveError> {
    let Some(seconds) = seconds else {
        return Ok(None);
    };
    let duration = Duration::try_from_secs_f64(seconds)
        .map_err(|_| ResolveError::Failed("invalid media duration".into()))?;
    u64::try_from(duration.as_millis())
        .map(Some)
        .map_err(|_| ResolveError::Failed("invalid media duration".into()))
}

pub(super) fn display_title(preferred: Option<&str>, final_url: &str) -> String {
    let candidate = preferred
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned)
        .or_else(|| {
            Url::parse(final_url)
                .ok()
                .and_then(|url| url.path_segments()?.next_back().map(str::to_owned))
        })
        .unwrap_or_else(|| "Audio".into());
    let sanitized: String = candidate
        .chars()
        .filter(|character| !character.is_control())
        .take(120)
        .collect();
    if sanitized.trim().is_empty() {
        "Audio".into()
    } else {
        sanitized
    }
}

pub(super) fn is_capacity_error(error: &IngestError) -> bool {
    matches!(
        error,
        IngestError::Fetch(
            FetchError::AdmissionCapacityExceeded | FetchError::Store(StoreError::CapacityExceeded)
        )
    )
}

pub(super) fn is_admission_capacity_error(error: &IngestError) -> bool {
    matches!(
        error,
        IngestError::Fetch(FetchError::AdmissionCapacityExceeded)
    )
}

pub(super) fn unsupported_media(source: pepeaudio_media::MediaSourceKind) -> ResolveError {
    if source == pepeaudio_media::MediaSourceKind::ResolvedSite {
        ResolveError::UnsupportedStream
    } else {
        ResolveError::UnsupportedAttachment
    }
}

pub(super) fn map_ingest(
    error: &IngestError,
    source: pepeaudio_media::MediaSourceKind,
) -> ResolveError {
    match error {
        IngestError::Fetch(
            FetchError::DeclaredSizeTooLarge
            | FetchError::ContentLengthTooLarge
            | FetchError::DownloadTooLarge,
        ) => ResolveError::TrackLimitExceeded,
        IngestError::Fetch(
            FetchError::AdmissionCapacityExceeded | FetchError::Store(StoreError::CapacityExceeded),
        ) => ResolveError::CapacityExceeded,
        IngestError::Probe(ProcessError::NoAudioStream | ProcessError::InvalidProbe) => {
            unsupported_media(source)
        }
        error => {
            tracing::warn!(error = %error, "managed media ingestion failed");
            ResolveError::Failed("media ingestion failed".into())
        }
    }
}
