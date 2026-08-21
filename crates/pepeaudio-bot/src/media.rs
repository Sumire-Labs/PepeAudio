use async_trait::async_trait;
use pepeaudio_core::{GuildId, UserId};
use pepeaudio_player::QueueTrack;
use thiserror::Error;

/// Safe subset of Discord attachment metadata passed to a resolver.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachmentSource {
    pub filename: String,
    pub url: String,
    pub content_type: Option<String>,
    pub size_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedMediaBatch {
    pub tracks: Vec<QueueTrack>,
    pub source_title: Option<String>,
    pub source_item_count: Option<usize>,
    pub skipped_items: usize,
    pub truncated: bool,
}

impl ResolvedMediaBatch {
    #[must_use]
    pub fn single(track: QueueTrack) -> Self {
        Self {
            tracks: vec![track],
            source_title: None,
            source_item_count: Some(1),
            skipped_items: 0,
            truncated: false,
        }
    }
}

/// Media resolution boundary; implementations enforce SSRF and download limits.
#[async_trait]
pub trait MediaResolver: Send + Sync + 'static {
    #[must_use]
    fn queue_capacity(&self) -> usize {
        usize::MAX
    }

    #[must_use]
    fn maximum_playlist_items(&self) -> usize {
        1
    }

    /// Resolves a supported URL or a bounded song-title search.
    async fn resolve_input(
        &self,
        guild_id: GuildId,
        requester: UserId,
        input: &str,
        maximum_items: usize,
    ) -> Result<ResolvedMediaBatch, ResolveError> {
        self.resolve_url(guild_id, requester, input, maximum_items)
            .await
    }

    /// Resolves a direct audio URL without downloading it in the command layer.
    async fn resolve_url(
        &self,
        guild_id: GuildId,
        requester: UserId,
        url: &str,
        maximum_items: usize,
    ) -> Result<ResolvedMediaBatch, ResolveError>;

    /// Downloads and validates a Discord attachment immediately while its
    /// signed CDN URL is valid.
    async fn resolve_attachment(
        &self,
        guild_id: GuildId,
        requester: UserId,
        attachment: AttachmentSource,
    ) -> Result<ResolvedMediaBatch, ResolveError>;

    /// Removes an uncommitted managed object after a command-path failure.
    ///
    /// Non-managed test resolvers may keep the default no-op behavior.
    async fn discard_uncommitted(&self, _batch: ResolvedMediaBatch) -> Result<(), ResolveError> {
        Ok(())
    }
}

/// Media resolution failure safe to surface to a command callback.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ResolveError {
    #[error("the media resolver adapter is not configured")]
    NotConfigured,
    #[error("media URL is not an allowed direct audio URL")]
    UnsupportedUrl,
    #[error("attachment is not an allowed audio file")]
    UnsupportedAttachment,
    #[error("YouTube and SoundCloud support is disabled")]
    SiteExtractorsDisabled,
    #[error("Spotify and Apple Music matching is disabled")]
    CrossServiceMatchingDisabled,
    #[error("the requested catalog provider is not configured")]
    CatalogProviderUnavailable,
    #[error("Spotify playlist import requires Spotify user authorization")]
    SpotifyPlaylistRequiresUserAuthorization,
    #[error("Spotify album import requires configured app credentials")]
    SpotifyAlbumRequiresCredentials,
    #[error("Apple Music playlist import requires Apple developer credentials")]
    AppleMusicPlaylistRequiresDeveloperCredentials,
    #[error("playlist contains more items than the configured limit")]
    PlaylistTooLarge,
    #[error("the provider did not expose one safe direct audio stream")]
    UnsupportedStream,
    #[error("no confidently matching YouTube or SoundCloud audio was found")]
    NoSearchMatch,
    #[error("site resolver is busy; try again shortly")]
    Busy,
    #[error("site media resolution exceeded its time limit")]
    TimedOut,
    #[error("this track exceeds the configured size or duration limit")]
    TrackLimitExceeded,
    #[error("managed media capacity is currently full")]
    CapacityExceeded,
    #[error("media resolution failed: {0}")]
    Failed(String),
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use pepeaudio_core::{GuildId, UserId};
    use pepeaudio_player::{PlaybackSource, QueueTrack};

    use super::{AttachmentSource, MediaResolver, ResolveError, ResolvedMediaBatch};

    struct FakeResolver;

    #[async_trait]
    impl MediaResolver for FakeResolver {
        async fn resolve_url(
            &self,
            _guild_id: GuildId,
            requester: UserId,
            url: &str,
            _maximum_items: usize,
        ) -> Result<ResolvedMediaBatch, ResolveError> {
            Ok(ResolvedMediaBatch::single(QueueTrack::new(
                "fake URL",
                Some(requester),
                Some(1_000),
                true,
                PlaybackSource::new(url),
            )))
        }

        async fn resolve_attachment(
            &self,
            _guild_id: GuildId,
            requester: UserId,
            attachment: AttachmentSource,
        ) -> Result<ResolvedMediaBatch, ResolveError> {
            Ok(ResolvedMediaBatch::single(QueueTrack::new(
                attachment.filename,
                Some(requester),
                None,
                false,
                PlaybackSource::new(attachment.url),
            )))
        }
    }

    #[tokio::test]
    async fn command_layer_can_test_resolution_without_network_or_discord() {
        let guild = GuildId::new(1).expect("guild");
        let user = UserId::new(2).expect("user");
        let batch = FakeResolver
            .resolve_url(guild, user, "memory://test", 1)
            .await
            .expect("fake resolves");
        let track = &batch.tracks[0];
        assert_eq!(track.title, "fake URL");
        assert_eq!(track.source.as_str(), "memory://test");
    }
}
