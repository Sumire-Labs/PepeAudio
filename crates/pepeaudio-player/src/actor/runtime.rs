use std::collections::VecDeque;

use pepeaudio_core::{GuildId, StateRevision};
use tokio::sync::{broadcast, mpsc};
use uuid::Uuid;

use crate::{
    PlaybackGeneration, PlaybackPort, PlayerConfig, PlayerError, PlayerEvent, ShutdownReport,
    SnapshotPublisher,
    model::PlayerModel,
    request::PlayerRequest,
    timer::{IdleExpired, IdleTimer},
};

pub(crate) struct Actor<P, S> {
    pub(super) model: PlayerModel,
    pub(super) playback: P,
    pub(super) publisher: S,
    pub(super) config: PlayerConfig,
    pub(super) requests: mpsc::Receiver<PlayerRequest>,
    pub(super) timer_sender: mpsc::Sender<IdleExpired>,
    pub(super) timers: mpsc::Receiver<IdleExpired>,
    pub(super) idle_timer: IdleTimer,
    pub(super) events: broadcast::Sender<PlayerEvent>,
    pub(super) recent_idempotency: VecDeque<Uuid>,
    pub(super) playback_generation: PlaybackGeneration,
}

impl<P, S> Actor<P, S>
where
    P: PlaybackPort,
    S: SnapshotPublisher,
{
    pub(crate) fn new(
        guild_id: GuildId,
        initial_revision: StateRevision,
        playback: P,
        publisher: S,
        config: PlayerConfig,
        requests: mpsc::Receiver<PlayerRequest>,
        events: broadcast::Sender<PlayerEvent>,
    ) -> Self {
        let (timer_sender, timers) = mpsc::channel(1);
        Self {
            model: PlayerModel::new(guild_id, initial_revision),
            playback,
            publisher,
            config,
            requests,
            timer_sender,
            timers,
            idle_timer: IdleTimer::new(),
            events,
            recent_idempotency: VecDeque::new(),
            playback_generation: PlaybackGeneration::default(),
        }
    }

    pub(crate) async fn run(mut self) -> ShutdownReport {
        let initial = self.current_snapshot();
        self.publish(&initial).await;

        loop {
            tokio::select! {
                request = self.requests.recv() => {
                    let Some(request) = request else {
                        break;
                    };
                    if let Some(report) = self.handle_request(request).await {
                        return report;
                    }
                }
                expiration = self.timers.recv() => {
                    if let Some(expiration) = expiration
                        && self.handle_idle_expiration(expiration).await
                    {
                        return self.graceful_shutdown().await;
                    }
                }
            }
        }

        self.graceful_shutdown().await
    }

    async fn handle_request(&mut self, request: PlayerRequest) -> Option<ShutdownReport> {
        match request {
            PlayerRequest::Connect {
                channel_id,
                expected_revision,
                reply,
            } => {
                let result = self.connect(channel_id, expected_revision).await;
                let _ = reply.send(result);
            }
            PlayerRequest::Enqueue {
                track,
                expected_revision,
                reply,
            } => {
                let result = self.enqueue(track, expected_revision).await;
                let _ = reply.send(result);
            }
            PlayerRequest::EnqueueBatch {
                tracks,
                expected_revision,
                reply,
            } => {
                let result = self.enqueue_batch(tracks, expected_revision).await;
                let _ = reply.send(result);
            }
            PlayerRequest::Apply { envelope, reply } => {
                let result = self.apply_command(envelope).await;
                let _ = reply.send(result);
            }
            PlayerRequest::PlaybackEnded {
                identity,
                reason,
                reply,
            } => {
                let result = self.playback_ended(identity, reason).await;
                let _ = reply.send(result);
            }
            PlayerRequest::ReconcileVoiceChannel { channel_id, reply } => {
                let result = self.reconcile_voice_channel(channel_id).await;
                let _ = reply.send(result);
            }
            PlayerRequest::Snapshot { reply } => {
                let _ = reply.send(self.current_snapshot());
            }
            PlayerRequest::Shutdown { reply } => {
                let report = self.graceful_shutdown().await;
                let _ = reply.send(report.clone());
                if report.disconnect_error.is_none() {
                    return Some(report);
                }
            }
        }
        None
    }

    pub(super) fn next_playback_generation(&mut self) -> Result<PlaybackGeneration, PlayerError> {
        let generation =
            self.playback_generation
                .next()
                .ok_or_else(|| PlayerError::TaskFailed {
                    message: "playback generation is exhausted".to_owned(),
                })?;
        self.playback_generation = generation;
        Ok(generation)
    }
}
