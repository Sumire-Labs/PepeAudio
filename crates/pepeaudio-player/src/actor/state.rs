use std::time::{SystemTime, UNIX_EPOCH};

use pepeaudio_core::{PlayerSnapshot, PlayerState, StateRevision, UnixTimeMillis};

use super::Actor;
use crate::{
    PlaybackPort, PlayerError, PlayerEvent, ShutdownReport, SideEffect, SnapshotPublisher,
    timer::IdleExpired,
};

const DISCONNECT_ATTEMPTS: usize = 3;
const DISCONNECT_RETRY_DELAY: std::time::Duration = std::time::Duration::from_millis(100);

impl<P, S> Actor<P, S>
where
    P: PlaybackPort,
    S: SnapshotPublisher,
{
    pub(super) fn current_snapshot(&self) -> PlayerSnapshot {
        self.model.snapshot(unix_now())
    }

    pub(super) fn check_revision(&self, expected: StateRevision) -> Result<(), PlayerError> {
        if expected == self.model.revision {
            Ok(())
        } else {
            Err(PlayerError::RevisionConflict {
                expected: expected.get(),
                actual: self.model.revision.get(),
            })
        }
    }

    pub(super) fn next_revision(&self) -> Result<StateRevision, PlayerError> {
        self.model
            .revision
            .checked_next()
            .ok_or(PlayerError::RevisionExhausted {
                revision: self.model.revision,
            })
    }

    pub(super) async fn finish_change(
        &mut self,
        was_idle: bool,
        next_revision: StateRevision,
    ) -> Result<PlayerSnapshot, PlayerError> {
        self.model.revision = next_revision;
        self.reconcile_idle(was_idle)?;
        let snapshot = self.current_snapshot();
        self.publish(&snapshot).await;
        Ok(snapshot)
    }

    pub(super) async fn publish(&mut self, snapshot: &PlayerSnapshot) {
        let _ = self
            .events
            .send(PlayerEvent::StateChanged(Box::new(snapshot.clone())));
        if let Err(error) = self.publisher.publish(snapshot).await {
            let _ = self.events.send(PlayerEvent::SnapshotPublicationFailed {
                revision: snapshot.revision,
                message: error.to_string(),
            });
        }
    }

    fn reconcile_idle(&mut self, was_idle: bool) -> Result<(), PlayerError> {
        let is_idle = self.model.is_idle_connected();
        if !was_idle && is_idle {
            self.arm_idle_timer()?;
        } else if was_idle && !is_idle && self.idle_timer.is_active() {
            let generation = self.idle_timer.cancel()?;
            let _ = self
                .events
                .send(PlayerEvent::IdleTimerCancelled { generation });
        }
        Ok(())
    }

    fn arm_idle_timer(&mut self) -> Result<(), PlayerError> {
        let generation = self
            .idle_timer
            .arm(self.config.idle_timeout(), self.timer_sender.clone())?;
        let _ = self.events.send(PlayerEvent::IdleTimerArmed {
            generation,
            timeout: self.config.idle_timeout(),
        });
        Ok(())
    }

    /// Returns `true` only after the current idle generation disconnected and
    /// committed its terminal snapshot. The actor exits after that transition.
    pub(super) async fn handle_idle_expiration(&mut self, expiration: IdleExpired) -> bool {
        let current = self.idle_timer.generation();
        if expiration.generation != current || !self.model.is_idle_connected() {
            let _ = self.events.send(PlayerEvent::StaleIdleTimerIgnored {
                expired: expiration.generation,
                current,
            });
            return false;
        }
        self.idle_timer.mark_fired(expiration.generation);

        let Ok(next_revision) = self.next_revision() else {
            return false;
        };
        if let Err(error) = self.playback.disconnect().await {
            let _ = self.events.send(PlayerEvent::BackgroundSideEffectFailed {
                operation: SideEffect::Disconnect,
                message: error.to_string(),
            });
            let _ = self.arm_idle_timer();
            return false;
        }

        let was_idle = self.model.is_idle_connected();
        self.model.voice_channel_id = None;
        self.model.state = PlayerState::Disconnected;
        self.model.current = None;
        self.model.queue.clear();
        self.model.history.clear();
        if self.finish_change(was_idle, next_revision).await.is_ok() {
            let _ = self.events.send(PlayerEvent::IdleDisconnected {
                generation: expiration.generation,
            });
            return true;
        }
        false
    }

    pub(super) async fn graceful_shutdown(&mut self) -> ShutdownReport {
        let disconnect_error = self.disconnect_for_shutdown().await;
        if disconnect_error.is_some() {
            return ShutdownReport {
                disconnect_error,
                final_revision: self.model.revision,
            };
        }

        if self.idle_timer.is_active()
            && let Ok(generation) = self.idle_timer.cancel()
        {
            let _ = self
                .events
                .send(PlayerEvent::IdleTimerCancelled { generation });
        }

        let needs_change = self.model.voice_channel_id.is_some()
            || self.model.current.is_some()
            || !self.model.queue.is_empty()
            || self.model.state != PlayerState::Disconnected;
        if needs_change && let Ok(next_revision) = self.next_revision() {
            let was_idle = self.model.is_idle_connected();
            self.model.voice_channel_id = None;
            self.model.state = PlayerState::Disconnected;
            self.model.current = None;
            self.model.queue.clear();
            self.model.history.clear();
            let _ = self.finish_change(was_idle, next_revision).await;
        }

        let report = ShutdownReport {
            disconnect_error,
            final_revision: self.model.revision,
        };
        let _ = self.events.send(PlayerEvent::Shutdown(report.clone()));
        report
    }

    async fn disconnect_for_shutdown(&mut self) -> Option<String> {
        self.model.voice_channel_id?;
        let mut last_error = None;
        for attempt in 0..DISCONNECT_ATTEMPTS {
            match self.playback.disconnect().await {
                Ok(()) => return None,
                Err(error) => last_error = Some(error.to_string()),
            }
            if attempt + 1 < DISCONNECT_ATTEMPTS {
                tokio::time::sleep(DISCONNECT_RETRY_DELAY).await;
            }
        }
        if let Some(message) = last_error.as_ref() {
            let _ = self.events.send(PlayerEvent::BackgroundSideEffectFailed {
                operation: SideEffect::Disconnect,
                message: message.clone(),
            });
        }
        last_error
    }
}

pub(super) fn unix_now() -> UnixTimeMillis {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    UnixTimeMillis::new(u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX))
}
