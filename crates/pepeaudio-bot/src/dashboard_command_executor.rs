use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use pepeaudio_core::{
    CommandEnvelope, CommandResultCode, GuildId, PlayerCommand, PlayerSnapshot, UnixTimeMillis,
};
use pepeaudio_player::{PlayerError, PlayerHandle};
use pepeaudio_runtime::{
    CommandAuthorization, CommandAuthorizer, CommandExecutionError, PlayerDirectory,
    WorkerPlayerError,
};

use crate::{
    MediaResolver, PlayerRegistry, ResolveError, ResolvedMediaBatch,
    commands::available_batch_items, web_authorizer::DiscordCommandAuthorizer,
};

pub(crate) struct DashboardCommandExecutor {
    players: Arc<PlayerRegistry>,
    media: Arc<dyn MediaResolver>,
    authorizer: Arc<DiscordCommandAuthorizer>,
}

impl DashboardCommandExecutor {
    pub(crate) fn new(
        players: Arc<PlayerRegistry>,
        media: Arc<dyn MediaResolver>,
        authorizer: Arc<DiscordCommandAuthorizer>,
    ) -> Self {
        Self {
            players,
            media,
            authorizer,
        }
    }

    async fn enqueue_media(
        &self,
        envelope: CommandEnvelope,
        input: String,
    ) -> Result<PlayerSnapshot, CommandExecutionError> {
        let actor = envelope
            .actor_user_id
            .ok_or(CommandExecutionError::Rejected(
                CommandResultCode::NotAuthorized,
            ))?;
        let player = self
            .players
            .get_or_create(envelope.guild_id)
            .await
            .map_err(|_| CommandExecutionError::Retryable)?;
        let before = player
            .snapshot()
            .await
            .map_err(CommandExecutionError::Player)?;
        envelope
            .validate_against(&before, unix_now())
            .map_err(PlayerError::from)?;
        let maximum_items = available_batch_items(self.media.as_ref(), &before)
            .map_err(|_| CommandExecutionError::Rejected(CommandResultCode::QueueFull))?;
        let batch = self
            .media
            .resolve_input(envelope.guild_id, actor, &input, maximum_items)
            .await
            .map_err(|error| CommandExecutionError::Rejected(media_result_code(&error)))?;

        let result = self.commit_batch(&envelope, actor, &player, &batch).await;
        if result.is_err() {
            self.discard(envelope.guild_id, batch).await;
        }
        result
    }

    async fn commit_batch(
        &self,
        envelope: &CommandEnvelope,
        actor: pepeaudio_core::UserId,
        player: &PlayerHandle,
        batch: &ResolvedMediaBatch,
    ) -> Result<PlayerSnapshot, CommandExecutionError> {
        match self.authorizer.authorize(envelope).await {
            CommandAuthorization::Allowed => {}
            CommandAuthorization::Denied => {
                return Err(CommandExecutionError::Rejected(
                    CommandResultCode::NotAuthorized,
                ));
            }
            CommandAuthorization::RetryableFailure => {
                return Err(CommandExecutionError::Retryable);
            }
        }
        let channel = self
            .authorizer
            .actor_voice_channel(envelope.guild_id, actor)
            .ok_or(CommandExecutionError::Rejected(
                CommandResultCode::NotAuthorized,
            ))?;
        let snapshot = player
            .snapshot()
            .await
            .map_err(CommandExecutionError::Player)?;
        if snapshot.revision != envelope.expected_revision {
            return Err(PlayerError::RevisionConflict {
                expected: envelope.expected_revision.get(),
                actual: snapshot.revision.get(),
            }
            .into());
        }
        match snapshot.voice_channel_id {
            None => {
                player
                    .connect(channel, snapshot.revision)
                    .await
                    .map_err(CommandExecutionError::Player)?;
            }
            Some(connected) if connected == channel => {}
            Some(connected) => {
                return Err(PlayerError::VoiceChannelMismatch {
                    connected,
                    requested: channel,
                }
                .into());
            }
        }
        let snapshot = player
            .snapshot()
            .await
            .map_err(CommandExecutionError::Player)?;
        player
            .enqueue_batch(batch.tracks.clone(), snapshot.revision)
            .await
            .map_err(CommandExecutionError::Player)
    }

    async fn discard(&self, guild_id: GuildId, batch: ResolvedMediaBatch) {
        if self.media.discard_uncommitted(batch).await.is_err() {
            tracing::warn!(
                guild_id = guild_id.get(),
                "dashboard media cleanup remains pending for the janitor"
            );
        }
    }
}

#[async_trait]
impl PlayerDirectory for DashboardCommandExecutor {
    async fn player(&self, guild_id: GuildId) -> Result<Option<PlayerHandle>, WorkerPlayerError> {
        self.players.player(guild_id).await
    }

    async fn execute(
        &self,
        envelope: CommandEnvelope,
    ) -> Result<PlayerSnapshot, CommandExecutionError> {
        let input = match &envelope.command {
            PlayerCommand::EnqueueMedia { input } => Some(input.clone()),
            _ => None,
        };
        if let Some(input) = input {
            self.enqueue_media(envelope, input).await
        } else {
            let player = self
                .player(envelope.guild_id)
                .await
                .map_err(|_| CommandExecutionError::Retryable)?
                .ok_or(CommandExecutionError::Retryable)?;
            player
                .apply(envelope)
                .await
                .map_err(CommandExecutionError::Player)
        }
    }
}

fn media_result_code(error: &ResolveError) -> CommandResultCode {
    match error {
        ResolveError::NoSearchMatch => CommandResultCode::MediaNotFound,
        ResolveError::Busy | ResolveError::TimedOut | ResolveError::CapacityExceeded => {
            CommandResultCode::MediaBusy
        }
        ResolveError::UnsupportedUrl
        | ResolveError::UnsupportedAttachment
        | ResolveError::SiteExtractorsDisabled
        | ResolveError::CrossServiceMatchingDisabled
        | ResolveError::CatalogProviderUnavailable
        | ResolveError::SpotifyPlaylistRequiresUserAuthorization
        | ResolveError::SpotifyAlbumRequiresCredentials
        | ResolveError::AppleMusicPlaylistRequiresDeveloperCredentials
        | ResolveError::PlaylistTooLarge
        | ResolveError::UnsupportedStream
        | ResolveError::TrackLimitExceeded => CommandResultCode::MediaUnsupported,
        ResolveError::NotConfigured | ResolveError::Failed(_) => CommandResultCode::MediaFailed,
    }
}

fn unix_now() -> UnixTimeMillis {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    UnixTimeMillis::new(u64::try_from(millis).unwrap_or(u64::MAX))
}
