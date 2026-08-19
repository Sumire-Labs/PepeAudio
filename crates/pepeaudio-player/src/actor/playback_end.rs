use std::collections::VecDeque;

use super::Actor;
use crate::{
    PlaybackEndReason, PlaybackGeneration, PlaybackIdentity, PlaybackPort, PlayerError,
    PlayerEvent, QueueTrack, SideEffect, SnapshotPublisher, model::ActiveTrack,
};
use pepeaudio_core::{PlayerSnapshot, PlayerState, RepeatMode};

impl<P, S> Actor<P, S>
where
    P: PlaybackPort,
    S: SnapshotPublisher,
{
    pub(super) async fn playback_ended(
        &mut self,
        identity: PlaybackIdentity,
        reason: PlaybackEndReason,
    ) -> Result<PlayerSnapshot, PlayerError> {
        let Some(active) = self.model.current.as_ref() else {
            return Ok(self.current_snapshot());
        };
        if active.identity() != identity {
            return Ok(self.current_snapshot());
        }

        let next_revision = self.next_revision()?;
        let was_idle = self.model.is_idle_connected();
        let finished = self
            .model
            .current
            .take()
            .expect("the matching active track was checked above")
            .track;
        let mut candidates = std::mem::take(&mut self.model.queue);
        let next = if reason.is_natural() {
            self.after_natural_end(finished, &mut candidates).await
        } else {
            self.first_playable(&mut candidates).await
        };

        self.model.queue = candidates;
        self.model.current = next;
        self.model.state = if self.model.current.is_some() {
            PlayerState::Playing
        } else {
            PlayerState::IdleConnected
        };
        self.finish_change(was_idle, next_revision).await
    }

    async fn after_natural_end(
        &mut self,
        finished: QueueTrack,
        candidates: &mut VecDeque<QueueTrack>,
    ) -> Option<ActiveTrack> {
        match self.model.repeat_mode {
            RepeatMode::Track => {
                if let Some(generation) = self.try_play(&finished).await {
                    return Some(ActiveTrack::playing(finished, generation));
                }
                self.push_history(finished);
            }
            RepeatMode::Queue if candidates.is_empty() => {
                if let Some(generation) = self.try_play(&finished).await {
                    return Some(ActiveTrack::playing(finished, generation));
                }
                self.push_history(finished);
            }
            RepeatMode::Queue => {
                self.push_history(finished.clone());
                candidates.push_back(finished);
            }
            RepeatMode::Off => self.push_history(finished),
        }
        self.first_playable(candidates).await
    }

    async fn first_playable(
        &mut self,
        candidates: &mut VecDeque<QueueTrack>,
    ) -> Option<ActiveTrack> {
        while let Some(candidate) = candidates.pop_front() {
            if let Some(generation) = self.try_play(&candidate).await {
                return Some(ActiveTrack::playing(candidate, generation));
            }
        }
        None
    }

    async fn try_play(&mut self, candidate: &QueueTrack) -> Option<PlaybackGeneration> {
        let generation = match self.next_playback_generation() {
            Ok(generation) => generation,
            Err(error) => {
                let _ = self.events.send(PlayerEvent::BackgroundSideEffectFailed {
                    operation: SideEffect::Play,
                    message: error.to_string(),
                });
                return None;
            }
        };
        match self.playback.play(candidate, generation).await {
            Ok(()) => Some(generation),
            Err(error) => {
                let _ = self.events.send(PlayerEvent::BackgroundSideEffectFailed {
                    operation: SideEffect::Play,
                    message: error.to_string(),
                });
                None
            }
        }
    }

    pub(super) fn push_history(&mut self, track: QueueTrack) {
        if self.model.history.len() == self.config.max_queued_tracks() {
            self.model.history.pop_front();
        }
        self.model.history.push_back(track);
    }
}
