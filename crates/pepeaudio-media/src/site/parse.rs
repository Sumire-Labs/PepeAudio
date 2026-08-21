use std::collections::BTreeMap;

use serde::Deserialize;

use crate::{MediaRequest, ResolvedSiteMedia, SafeHttpHeaders};

use super::{
    SiteCollection, SiteError, SiteProvider, SiteReference, SiteResolvedTrack, YtDlpConfig,
};

#[derive(Deserialize)]
struct RawInfo {
    #[serde(rename = "_type")]
    kind: Option<String>,
    title: Option<String>,
    id: Option<String>,
    url: Option<String>,
    webpage_url: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    uploader: Option<String>,
    channel: Option<String>,
    entries: Option<Vec<Option<RawInfo>>>,
    playlist_count: Option<usize>,
    duration: Option<f64>,
    protocol: Option<String>,
    vcodec: Option<String>,
    acodec: Option<String>,
    is_live: Option<bool>,
    live_status: Option<String>,
    #[serde(default)]
    http_headers: BTreeMap<String, String>,
}

pub(crate) fn collection(
    bytes: &[u8],
    provider: SiteProvider,
    original_url: &str,
    maximum: usize,
) -> Result<SiteCollection, SiteError> {
    collection_with_empty_error(bytes, provider, original_url, maximum, false)
}

pub(crate) fn search_collection(
    bytes: &[u8],
    provider: SiteProvider,
    original_url: &str,
    maximum: usize,
) -> Result<SiteCollection, SiteError> {
    collection_with_empty_error(bytes, provider, original_url, maximum, true)
}

fn collection_with_empty_error(
    bytes: &[u8],
    provider: SiteProvider,
    original_url: &str,
    maximum: usize,
    empty_is_no_match: bool,
) -> Result<SiteCollection, SiteError> {
    let raw: RawInfo = serde_json::from_slice(bytes).map_err(|_| SiteError::InvalidMetadata)?;
    let is_collection = raw.kind.as_deref() == Some("playlist") || raw.entries.is_some();
    let entries = if is_collection {
        let entries = raw.entries.ok_or(SiteError::InvalidMetadata)?;
        let observed_items = entries.len();
        let truncated =
            observed_items > maximum || raw.playlist_count.is_some_and(|count| count > maximum);
        let source_item_count = raw
            .playlist_count
            .or_else(|| (!truncated).then_some(observed_items));
        let mut skipped_items = 0;
        let entries = entries
            .into_iter()
            .take(maximum)
            .filter_map(|entry| {
                if let Some(reference) = entry.and_then(|entry| reference(&entry, provider).ok()) {
                    Some(reference)
                } else {
                    skipped_items += 1;
                    None
                }
            })
            .collect::<Vec<_>>();
        return finish_collection(
            raw.title,
            entries,
            source_item_count,
            skipped_items,
            truncated,
            empty_is_no_match,
        );
    } else {
        vec![validated_reference(provider, original_url)?]
    };
    if entries.is_empty() {
        return Err(SiteError::InvalidMetadata);
    }
    Ok(SiteCollection {
        title: raw.title.and_then(clean_title),
        entries,
        source_item_count: Some(1),
        skipped_items: 0,
        truncated: false,
    })
}

fn finish_collection(
    title: Option<String>,
    entries: Vec<SiteReference>,
    source_item_count: Option<usize>,
    skipped_items: usize,
    truncated: bool,
    empty_is_no_match: bool,
) -> Result<SiteCollection, SiteError> {
    if entries.is_empty() {
        return Err(match (empty_is_no_match, skipped_items) {
            (true, _) => SiteError::NoSearchMatch,
            (false, 1..) => SiteError::UnsupportedStream,
            (false, 0) => SiteError::InvalidMetadata,
        });
    }
    Ok(SiteCollection {
        title: title.and_then(clean_title),
        entries,
        source_item_count,
        skipped_items,
        truncated,
    })
}

pub(crate) fn resolved(
    bytes: &[u8],
    reference: &SiteReference,
    config: &YtDlpConfig,
) -> Result<SiteResolvedTrack, SiteError> {
    let raw: RawInfo = serde_json::from_slice(bytes).map_err(|_| SiteError::InvalidMetadata)?;
    resolved_info(raw, reference, config)
}

pub(crate) fn resolved_search(
    bytes: &[u8],
    provider: SiteProvider,
    config: &YtDlpConfig,
) -> Result<SiteResolvedTrack, SiteError> {
    let mut raw: RawInfo = serde_json::from_slice(bytes).map_err(|_| SiteError::InvalidMetadata)?;
    if raw.kind.as_deref() == Some("playlist") || raw.entries.is_some() {
        raw = raw
            .entries
            .take()
            .and_then(|entries| entries.into_iter().flatten().next())
            .ok_or(SiteError::NoSearchMatch)?;
    }
    let reference = reference_from_raw(&raw, provider)?;
    resolved_info(raw, &reference, config)
}

fn resolved_info(
    raw: RawInfo,
    reference: &SiteReference,
    config: &YtDlpConfig,
) -> Result<SiteResolvedTrack, SiteError> {
    let provider = reference.provider;
    if raw.is_live.unwrap_or(false)
        || !matches!(
            raw.live_status.as_deref(),
            None | Some("not_live" | "was_live")
        )
        || !matches!(raw.protocol.as_deref(), Some("http" | "https"))
        || raw.vcodec.as_deref() != Some("none")
        || matches!(raw.acodec.as_deref(), None | Some("none"))
    {
        return Err(SiteError::UnsupportedStream);
    }
    let duration = raw.duration.ok_or(SiteError::InvalidMetadata)?;
    let duration =
        std::time::Duration::try_from_secs_f64(duration).map_err(|_| SiteError::InvalidMetadata)?;
    if duration.is_zero() || duration > config.maximum_track_duration {
        return Err(SiteError::DurationLimit);
    }
    let duration_ms = u64::try_from(duration.as_millis()).map_err(|_| SiteError::DurationLimit)?;
    let url = raw.url.ok_or(SiteError::UnsupportedStream)?;
    validate_media_url(&url, provider)?;
    let headers = SafeHttpHeaders::from_ytdlp(raw.http_headers, provider)?;
    let page_url = canonical_page_url(
        provider,
        raw.id.as_deref(),
        raw.webpage_url.as_deref().unwrap_or(reference.page_url()),
    )?;
    let title = raw
        .title
        .and_then(clean_title)
        .ok_or(SiteError::InvalidMetadata)?;
    let artist = raw
        .artist
        .or(raw.uploader)
        .or(raw.channel)
        .and_then(clean_title);
    let album = raw.album.and_then(clean_title);
    Ok(SiteResolvedTrack {
        title,
        artist,
        album,
        provider,
        page_url,
        duration_ms,
        request: MediaRequest::ResolvedSite(ResolvedSiteMedia::new(url, provider, headers)),
    })
}

fn canonical_page_url(
    provider: SiteProvider,
    id: Option<&str>,
    candidate: &str,
) -> Result<String, SiteError> {
    match provider {
        SiteProvider::YouTube => canonical_youtube_page(id, candidate),
        SiteProvider::SoundCloud => canonical_soundcloud_page(candidate),
    }
}

fn canonical_youtube_page(id: Option<&str>, candidate: &str) -> Result<String, SiteError> {
    let candidate_url = url::Url::parse(candidate).map_err(|_| SiteError::InvalidMetadata)?;
    let id = id
        .filter(|id| valid_youtube_id(id))
        .map(str::to_owned)
        .or_else(|| match candidate_url.host_str() {
            Some("youtu.be") => candidate_url
                .path_segments()
                .and_then(|mut segments| segments.next())
                .filter(|id| valid_youtube_id(id))
                .map(str::to_owned),
            Some(host) if SiteProvider::YouTube.accepts_page_host(host) => {
                candidate_url.query_pairs().find_map(|(name, value)| {
                    (name == "v" && valid_youtube_id(&value)).then(|| value.into_owned())
                })
            }
            _ => None,
        })
        .ok_or(SiteError::InvalidMetadata)?;
    Ok(format!("https://www.youtube.com/watch?v={id}"))
}

fn canonical_soundcloud_page(candidate: &str) -> Result<String, SiteError> {
    let mut url = url::Url::parse(candidate).map_err(|_| SiteError::InvalidMetadata)?;
    if SiteProvider::classify(candidate)? != Some(SiteProvider::SoundCloud)
        || url.host_str() == Some("on.soundcloud.com")
        || url
            .path_segments()
            .is_none_or(|segments| segments.filter(|part| !part.is_empty()).count() != 2)
    {
        return Err(SiteError::InvalidMetadata);
    }
    url.set_query(None);
    Ok(url.to_string())
}

fn valid_youtube_id(id: &str) -> bool {
    id.len() == 11
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn reference(raw: &RawInfo, provider: SiteProvider) -> Result<SiteReference, SiteError> {
    reference_from_raw(raw, provider)
}

fn reference_from_raw(raw: &RawInfo, provider: SiteProvider) -> Result<SiteReference, SiteError> {
    let title = raw.title.clone().and_then(clean_title);
    let artist = raw
        .artist
        .clone()
        .or_else(|| raw.uploader.clone())
        .or_else(|| raw.channel.clone())
        .and_then(clean_title);
    let duration_ms = raw.duration.and_then(finite_duration_ms);
    if let Some(url) = raw.webpage_url.as_ref().or(raw.url.as_ref())
        && let Ok(mut reference) = validated_reference(provider, url)
    {
        reference.title = title;
        reference.artist = artist;
        reference.duration_ms = duration_ms;
        return Ok(reference);
    }
    if provider == SiteProvider::YouTube {
        let id = raw.id.as_deref().ok_or(SiteError::InvalidMetadata)?;
        if valid_youtube_id(id) {
            let mut reference =
                validated_reference(provider, &format!("https://www.youtube.com/watch?v={id}"))?;
            reference.title = title;
            reference.artist = artist;
            reference.duration_ms = duration_ms;
            return Ok(reference);
        }
    }
    Err(SiteError::InvalidMetadata)
}

fn validated_reference(provider: SiteProvider, raw: &str) -> Result<SiteReference, SiteError> {
    if SiteProvider::classify(raw)? != Some(provider) {
        return Err(SiteError::InvalidMetadata);
    }
    Ok(SiteReference {
        provider,
        page_url: raw.to_owned(),
        title: None,
        artist: None,
        duration_ms: None,
    })
}

fn finite_duration_ms(seconds: f64) -> Option<u64> {
    let duration = std::time::Duration::try_from_secs_f64(seconds).ok()?;
    u64::try_from(duration.as_millis()).ok()
}

fn validate_media_url(raw: &str, provider: SiteProvider) -> Result<(), SiteError> {
    let url = url::Url::parse(raw).map_err(|_| SiteError::UnsupportedStream)?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || url.port_or_known_default() != Some(443)
        || !provider.accepts_media_host(url.host_str().unwrap_or_default())
    {
        return Err(SiteError::UnsupportedStream);
    }
    Ok(())
}

fn clean_title(mut value: String) -> Option<String> {
    value.retain(|character| !character.is_control());
    if let Some((byte_index, _)) = value.char_indices().nth(120) {
        value.truncate(byte_index);
    }
    (!value.trim().is_empty()).then_some(value)
}
