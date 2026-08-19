use pepeaudio_core::{ChannelId, PlayerSnapshot, PlayerState};

use super::Actor;
use crate::{PlaybackPort, PlayerError, PlayerEvent, SideEffect, SnapshotPublisher};

impl<P, S> Actor<P, S>
where
    P: PlaybackPort,
    S: SnapshotPublisher,
{
    pub(super) async fn reconcile_voice_channel(
        &mut self,
        observed_channel: Option<ChannelId>,
    ) -> Result<PlayerSnapshot, PlayerError> {
        if self.model.voice_channel_id == observed_channel {
            return Ok(self.current_snapshot());
        }

        let next_revision = self.next_revision()?;
        let was_idle = self.model.is_idle_connected();
        match observed_channel {
            Some(channel) if self.connect_observed_channel(channel).await => {
                self.model.voice_channel_id = Some(channel);
                if self.model.current.is_none() {
                    self.model.state = PlayerState::IdleConnected;
                }
            }
            Some(_) | None => {
                if observed_channel.is_none() {
                    self.disconnect_best_effort().await;
                }
                self.clear_disconnected();
            }
        }
        self.finish_change(was_idle, next_revision).await
    }

    async fn connect_observed_channel(&mut self, channel: ChannelId) -> bool {
        if let Err(error) = self.playback.connect(channel).await {
            self.report_background(SideEffect::Connect, &error);
            self.disconnect_best_effort().await;
            return false;
        }
        true
    }

    async fn disconnect_best_effort(&mut self) {
        if let Err(error) = self.playback.disconnect().await {
            self.report_background(SideEffect::Disconnect, &error);
        }
    }

    fn clear_disconnected(&mut self) {
        self.model.voice_channel_id = None;
        self.model.current = None;
        self.model.queue.clear();
        self.model.history.clear();
        self.model.state = PlayerState::Disconnected;
    }

    fn report_background(&self, operation: SideEffect, error: &P::Error) {
        let _ = self.events.send(PlayerEvent::BackgroundSideEffectFailed {
            operation,
            message: error.to_string(),
        });
    }
}
