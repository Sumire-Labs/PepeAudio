use std::{collections::VecDeque, fmt, sync::Arc};

use pepeaudio_core::{
    ChannelId, GuildId, HrirPresetId, MAX_TRACK_TITLE_BYTES, PlayerSnapshot, PlayerState,
    RepeatMode, StateRevision, TrackSnapshot, UnixTimeMillis, UserId, Volume,
};
use tokio::time::Instant;
use uuid::Uuid;

use crate::QueueTrackMetadata;
use crate::{PlaybackGeneration, PlaybackIdentity};

/// An opaque media locator passed only to the playback adapter.
///
/// It is intentionally absent from [`PlayerSnapshot`] so signed URLs and local
/// storage paths cannot leak through Discord or dashboard state publication.
#[derive(Clone)]
pub struct PlaybackSource {
    locator: String,
    lease: Option<Arc<dyn OpaquePlaybackLease>>,
}

trait OpaquePlaybackLease: Send + Sync {}

impl<T: Send + Sync> OpaquePlaybackLease for T {}

impl PlaybackSource {
    #[must_use]
    pub fn new(locator: impl Into<String>) -> Self {
        Self {
            locator: locator.into(),
            lease: None,
        }
    }

    /// The guard is deliberately type-erased and inaccessible after this
    /// boundary. Cloning the source shares the same guard without changing
    /// locator equality or exposing the guard through player snapshots.
    #[must_use]
    pub fn with_lease(locator: impl Into<String>, lease: impl Send + Sync + 'static) -> Self {
        Self {
            locator: locator.into(),
            lease: Some(Arc::new(lease)),
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.locator
    }
}

impl fmt::Debug for PlaybackSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PlaybackSource")
            .field("locator", &"<opaque>")
            .field("leased", &self.lease.is_some())
            .finish()
    }
}

impl PartialEq for PlaybackSource {
    fn eq(&self, other: &Self) -> bool {
        self.locator == other.locator
    }
}

impl Eq for PlaybackSource {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueueTrack {
    pub track_id: Uuid,
    pub title: String,
    pub requester_user_id: Option<UserId>,
    pub duration_ms: Option<u64>,
    pub seekable: bool,
    pub source: PlaybackSource,
    metadata: QueueTrackMetadata,
}

impl QueueTrack {
    /// Assigns a fresh track identity.
    #[must_use]
    pub fn new(
        title: impl Into<String>,
        requester_user_id: Option<UserId>,
        duration_ms: Option<u64>,
        seekable: bool,
        source: PlaybackSource,
    ) -> Self {
        Self {
            track_id: Uuid::new_v4(),
            title: title.into(),
            requester_user_id,
            duration_ms,
            seekable,
            source,
            metadata: QueueTrackMetadata::default(),
        }
    }

    /// Attaches validated display metadata and stable public provenance pages.
    #[must_use]
    pub fn with_metadata(mut self, metadata: QueueTrackMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    #[must_use]
    pub const fn metadata(&self) -> &QueueTrackMetadata {
        &self.metadata
    }

    pub(crate) fn has_valid_public_title(&self) -> bool {
        !self.title.trim().is_empty()
            && self.title.len() <= MAX_TRACK_TITLE_BYTES
            && !self.title.chars().any(char::is_control)
    }

    pub(crate) fn snapshot(&self, position_ms: u64) -> TrackSnapshot {
        TrackSnapshot {
            track_id: self.track_id,
            title: self.title.clone(),
            artist: self.metadata.artist().map(str::to_owned),
            album: self.metadata.album().map(str::to_owned),
            provenance: self.metadata.provenance().cloned(),
            requester_user_id: self.requester_user_id,
            duration_ms: self.duration_ms,
            position_ms,
            seekable: self.seekable,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ActiveTrack {
    pub(crate) track: QueueTrack,
    generation: PlaybackGeneration,
    pub(crate) position_ms: u64,
    advancing_since: Option<Instant>,
}

impl ActiveTrack {
    pub(crate) fn playing(track: QueueTrack, generation: PlaybackGeneration) -> Self {
        Self {
            track,
            generation,
            position_ms: 0,
            advancing_since: Some(Instant::now()),
        }
    }

    pub(crate) fn pause(&mut self) {
        self.position_ms = self.current_position_ms();
        self.advancing_since = None;
    }

    pub(crate) fn resume(&mut self) {
        self.advancing_since = Some(Instant::now());
    }

    pub(crate) fn seek(
        &mut self,
        position_ms: u64,
        advancing: bool,
        generation: PlaybackGeneration,
    ) {
        self.position_ms = position_ms;
        self.advancing_since = advancing.then(Instant::now);
        self.generation = generation;
    }

    pub(crate) fn identity(&self) -> PlaybackIdentity {
        PlaybackIdentity::new(self.track.track_id, self.generation)
    }

    fn current_position_ms(&self) -> u64 {
        let elapsed = self.advancing_since.map_or(0, |started| {
            u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
        });
        let position = self.position_ms.saturating_add(elapsed);
        self.track
            .duration_ms
            .map_or(position, |duration| position.min(duration))
    }

    fn snapshot(&self) -> TrackSnapshot {
        self.track.snapshot(self.current_position_ms())
    }
}

pub(crate) struct PlayerModel {
    pub(crate) guild_id: GuildId,
    pub(crate) voice_channel_id: Option<ChannelId>,
    pub(crate) revision: StateRevision,
    pub(crate) state: PlayerState,
    pub(crate) current: Option<ActiveTrack>,
    pub(crate) queue: VecDeque<QueueTrack>,
    pub(crate) history: VecDeque<QueueTrack>,
    pub(crate) volume: Volume,
    pub(crate) repeat_mode: RepeatMode,
    pub(crate) shuffle_enabled: bool,
    pub(crate) hrir_preset: Option<HrirPresetId>,
    pub(crate) spatial_audio_enabled: bool,
}

impl PlayerModel {
    pub(crate) fn new(guild_id: GuildId, initial_revision: StateRevision) -> Self {
        Self {
            guild_id,
            voice_channel_id: None,
            revision: initial_revision,
            state: PlayerState::Disconnected,
            current: None,
            queue: VecDeque::new(),
            history: VecDeque::new(),
            volume: Volume::DEFAULT,
            repeat_mode: RepeatMode::Off,
            shuffle_enabled: false,
            hrir_preset: None,
            spatial_audio_enabled: false,
        }
    }

    pub(crate) fn is_idle_connected(&self) -> bool {
        self.voice_channel_id.is_some()
            && self.state == PlayerState::IdleConnected
            && self.current.is_none()
            && self.queue.is_empty()
    }

    pub(crate) fn contains_track(&self, track_id: Uuid) -> bool {
        self.current
            .as_ref()
            .is_some_and(|active| active.track.track_id == track_id)
            || self.queue.iter().any(|track| track.track_id == track_id)
    }

    pub(crate) fn snapshot(&self, observed_at: UnixTimeMillis) -> PlayerSnapshot {
        PlayerSnapshot {
            guild_id: self.guild_id,
            voice_channel_id: self.voice_channel_id,
            revision: self.revision,
            state: self.state,
            current_track: self.current.as_ref().map(ActiveTrack::snapshot),
            queued_tracks: u32::try_from(self.queue.len()).unwrap_or(u32::MAX),
            upcoming_tracks: self.queue.iter().map(|track| track.snapshot(0)).collect(),
            has_previous_track: !self.history.is_empty(),
            volume: self.volume,
            repeat_mode: self.repeat_mode,
            shuffle_enabled: self.shuffle_enabled,
            hrir_preset: self.hrir_preset.clone(),
            spatial_audio_enabled: self.spatial_audio_enabled,
            observed_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use pepeaudio_core::{MediaProvider, PublicMediaPage, TrackProvenance};

    use super::{PlaybackSource, QueueTrack};
    use crate::QueueTrackMetadata;

    struct DropMarker(Arc<AtomicUsize>);

    impl Drop for DropMarker {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn queue_track_clones_share_one_opaque_source_lease() {
        let drops = Arc::new(AtomicUsize::new(0));
        let source = PlaybackSource::with_lease(
            "C:/private/media/0000000000000000000000000000000a",
            DropMarker(drops.clone()),
        );
        let track = QueueTrack::new("leased", None, Some(1_000), true, source);
        let clone = track.clone();

        assert!(!format!("{track:?}").contains("C:/private/media"));
        drop(track);
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        drop(clone);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn a_lease_does_not_change_source_locator_equality() {
        let leased = PlaybackSource::with_lease("memory://same", ());
        let fake = PlaybackSource::new("memory://same");

        assert_eq!(leased, fake);
        assert_eq!(leased.as_str(), "memory://same");
    }

    #[test]
    fn snapshot_contains_display_metadata_but_never_the_playback_locator() {
        let origin = PublicMediaPage::new(
            MediaProvider::Spotify,
            "https://open.spotify.com/track/4uLU6hMCjMI75M1A2tKUQC",
        )
        .expect("origin page");
        let playback = PublicMediaPage::new(
            MediaProvider::YouTube,
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
        )
        .expect("playback page");
        let provenance =
            TrackProvenance::new(Some(origin), playback).expect("valid playback provider");
        let metadata = QueueTrackMetadata::builder()
            .artist("Rick Astley")
            .expect("artist")
            .album("Whenever You Need Somebody")
            .expect("album")
            .provenance(provenance)
            .build();
        let source = PlaybackSource::new(
            "https://rr1---sn.example.googlevideo.com/videoplayback?sig=do-not-expose",
        );
        let track = QueueTrack::new("Never Gonna Give You Up", None, Some(213_573), true, source)
            .with_metadata(metadata);

        let json = serde_json::to_string(&track.snapshot(10_000)).expect("snapshot JSON");

        assert!(json.contains("Rick Astley"));
        assert!(json.contains("youtube.com/watch"));
        assert!(!json.contains("googlevideo"));
        assert!(!json.contains("do-not-expose"));
    }
}
