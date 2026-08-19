use crate::{
    PlaybackEndReason, PlaybackIdentity, PlaybackPort, PlayerConfig, PlayerError, PlayerEvent,
    QueueTrack, ShutdownReport, SnapshotPublisher, actor::Actor, request::PlayerRequest,
};
use pepeaudio_core::{ChannelId, CommandEnvelope, GuildId, PlayerSnapshot, StateRevision};
use tokio::{
    sync::{broadcast, mpsc, oneshot},
    task::JoinHandle,
};

/// Cloneable, bounded mailbox handle for one guild player.
pub struct PlayerHandle {
    requests: mpsc::Sender<PlayerRequest>,
    events: broadcast::Receiver<PlayerEvent>,
}

impl Clone for PlayerHandle {
    fn clone(&self) -> Self {
        Self {
            requests: self.requests.clone(),
            events: self.events.resubscribe(),
        }
    }
}

impl PlayerHandle {
    /// Subscribes to best-effort lifecycle events.
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<PlayerEvent> {
        self.events.resubscribe()
    }

    /// # Errors
    ///
    /// Returns [`PlayerError::ActorStopped`] after the actor exits.
    pub async fn snapshot(&self) -> Result<PlayerSnapshot, PlayerError> {
        let (reply, response) = oneshot::channel();
        self.requests
            .send(PlayerRequest::Snapshot { reply })
            .await
            .map_err(|_| PlayerError::ActorStopped)?;
        response.await.map_err(|_| PlayerError::ActorStopped)
    }

    /// # Errors
    ///
    /// Returns [`PlayerError`] when the revision, channel ownership, adapter,
    /// or actor lifecycle rejects the request.
    pub async fn connect(
        &self,
        channel_id: ChannelId,
        expected_revision: StateRevision,
    ) -> Result<PlayerSnapshot, PlayerError> {
        self.mutate(|reply| PlayerRequest::Connect {
            channel_id,
            expected_revision,
            reply,
        })
        .await
    }

    /// Starts the track immediately when nothing is active.
    ///
    /// # Errors
    ///
    /// Returns [`PlayerError`] when the request is stale, disconnected,
    /// duplicate, over capacity, or rejected by the adapter or actor.
    pub async fn enqueue(
        &self,
        track: QueueTrack,
        expected_revision: StateRevision,
    ) -> Result<PlayerSnapshot, PlayerError> {
        self.mutate(|reply| PlayerRequest::Enqueue {
            track,
            expected_revision,
            reply,
        })
        .await
    }

    /// Adds an ordered collection with one authoritative revision change.
    ///
    /// The first item starts immediately when the player is idle. Capacity,
    /// duplicate identities, and the first playback side effect are validated
    /// before any actor state is committed.
    ///
    /// # Errors
    ///
    /// Returns [`PlayerError`] when the request is stale, disconnected,
    /// contains duplicate tracks, exceeds capacity, or cannot start playback.
    pub async fn enqueue_batch(
        &self,
        tracks: Vec<QueueTrack>,
        expected_revision: StateRevision,
    ) -> Result<PlayerSnapshot, PlayerError> {
        self.mutate(|reply| PlayerRequest::EnqueueBatch {
            tracks,
            expected_revision,
            reply,
        })
        .await
    }

    /// # Errors
    ///
    /// Returns [`PlayerError`] when validation, a playback side effect, or the
    /// actor lifecycle rejects the command.
    pub async fn apply(&self, envelope: CommandEnvelope) -> Result<PlayerSnapshot, PlayerError> {
        self.mutate(|reply| PlayerRequest::Apply { envelope, reply })
            .await
    }

    /// # Errors
    ///
    /// Returns [`PlayerError`] when the transition, adapter, or actor fails.
    pub async fn playback_ended(
        &self,
        identity: PlaybackIdentity,
        reason: PlaybackEndReason,
    ) -> Result<PlayerSnapshot, PlayerError> {
        self.mutate(|reply| PlayerRequest::PlaybackEnded {
            identity,
            reason,
            reply,
        })
        .await
    }

    /// This does not require an optimistic revision because it describes an
    /// external fact rather than a user mutation. A disconnected observation
    /// clears live playback even when adapter cleanup reports an error.
    ///
    /// # Errors
    ///
    /// Returns [`PlayerError`] when the revision cannot advance or the actor stops.
    pub async fn reconcile_voice_channel(
        &self,
        channel_id: Option<ChannelId>,
    ) -> Result<PlayerSnapshot, PlayerError> {
        self.mutate(|reply| PlayerRequest::ReconcileVoiceChannel { channel_id, reply })
            .await
    }

    /// Requests graceful cleanup. Once queued, cleanup continues even if this
    /// caller is cancelled while awaiting the response. A report containing a
    /// disconnect error leaves the actor live so the same handle can retry.
    ///
    /// # Errors
    ///
    /// Returns [`PlayerError::ActorStopped`] when cleanup cannot be requested.
    pub async fn shutdown(&self) -> Result<ShutdownReport, PlayerError> {
        let (reply, response) = oneshot::channel();
        self.requests
            .send(PlayerRequest::Shutdown { reply })
            .await
            .map_err(|_| PlayerError::ActorStopped)?;
        response.await.map_err(|_| PlayerError::ActorStopped)
    }

    async fn mutate(
        &self,
        build: impl FnOnce(crate::request::MutationReply) -> PlayerRequest,
    ) -> Result<PlayerSnapshot, PlayerError> {
        let (reply, response) = oneshot::channel();
        self.requests
            .send(build(reply))
            .await
            .map_err(|_| PlayerError::ActorStopped)?;
        response.await.map_err(|_| PlayerError::ActorStopped)?
    }
}

pub struct PlayerRuntime {
    handle: PlayerHandle,
    task: JoinHandle<ShutdownReport>,
}

impl PlayerRuntime {
    #[must_use]
    pub fn handle(&self) -> PlayerHandle {
        self.handle.clone()
    }

    /// # Errors
    ///
    /// Returns [`PlayerError::TaskFailed`] if the actor task panics or is aborted.
    pub async fn shutdown(self) -> Result<ShutdownReport, PlayerError> {
        let _requested = self.handle.shutdown().await;
        drop(self.handle);
        match self.task.await {
            Ok(report) => Ok(report),
            Err(error) => Err(PlayerError::TaskFailed {
                message: error.to_string(),
            }),
        }
    }
}

#[must_use]
pub fn spawn_player<P, S>(
    guild_id: GuildId,
    config: PlayerConfig,
    playback: P,
    publisher: S,
) -> PlayerRuntime
where
    P: PlaybackPort,
    S: SnapshotPublisher,
{
    spawn_player_with_revision(
        guild_id,
        StateRevision::INITIAL,
        config,
        playback,
        publisher,
    )
}

/// Spawns a player actor from a durable monotonic revision watermark.
///
/// The state itself is deliberately not hydrated: voice calls and managed
/// media are process-local. Only the revision clock survives a process restart.
#[must_use]
pub fn spawn_player_with_revision<P, S>(
    guild_id: GuildId,
    initial_revision: StateRevision,
    config: PlayerConfig,
    playback: P,
    publisher: S,
) -> PlayerRuntime
where
    P: PlaybackPort,
    S: SnapshotPublisher,
{
    let (requests, receiver) = mpsc::channel(config.command_capacity());
    let (events, _) = broadcast::channel(config.event_capacity());
    let event_receiver = events.subscribe();
    let actor = Actor::new(
        guild_id,
        initial_revision,
        playback,
        publisher,
        config,
        receiver,
        events.clone(),
    );
    let task = tokio::spawn(actor.run());

    PlayerRuntime {
        handle: PlayerHandle {
            requests,
            events: event_receiver,
        },
        task,
    }
}
