use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use pepeaudio_components_v2::Message;
use pepeaudio_core::{
    GuildId, PlayerSnapshot, PlayerState, RepeatMode, StateRevision, TrackSnapshot, UnixTimeMillis,
    Volume,
};
use serenity::model::id::{ApplicationId, ChannelId, InteractionId, MessageId};
use uuid::Uuid;

use super::{NowPanelUpdater, SnapshotSource};
use crate::{ComponentIdCodec, ComponentsV2Responder, RestBoundaryError};

struct FixedSource(PlayerSnapshot);

#[async_trait]
impl SnapshotSource for FixedSource {
    async fn snapshot(&self) -> Option<PlayerSnapshot> {
        Some(self.0.clone())
    }
}

#[derive(Default)]
struct RecordingResponder {
    edits: Mutex<Vec<(u64, u64)>>,
}

#[async_trait]
impl ComponentsV2Responder for RecordingResponder {
    async fn create_response(
        &self,
        _interaction_id: InteractionId,
        _interaction_token: &str,
        _payload: &Message,
    ) -> Result<(), RestBoundaryError> {
        Ok(())
    }

    async fn edit_original_response(
        &self,
        _application_id: ApplicationId,
        _interaction_token: &str,
        _payload: &Message,
    ) -> Result<MessageId, RestBoundaryError> {
        Ok(MessageId::new(1))
    }

    async fn edit_message(
        &self,
        channel_id: ChannelId,
        message_id: MessageId,
        _payload: &Message,
    ) -> Result<(), RestBoundaryError> {
        self.edits
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((channel_id.get(), message_id.get()));
        Ok(())
    }
}

#[tokio::test(start_paused = true)]
async fn only_the_latest_panel_for_a_guild_is_refreshed() {
    let responder = Arc::new(RecordingResponder::default());
    let updater = NowPanelUpdater::new(
        responder.clone(),
        ComponentIdCodec::new([7; 32]).expect("component codec"),
        Arc::from([]),
    );
    let initial = snapshot(0);
    updater.track_source(
        initial.guild_id,
        ChannelId::new(10),
        MessageId::new(100),
        Arc::new(FixedSource(snapshot(10_000))),
        &initial,
    );
    updater.track_source(
        initial.guild_id,
        ChannelId::new(20),
        MessageId::new(200),
        Arc::new(FixedSource(snapshot(10_000))),
        &initial,
    );
    tokio::task::yield_now().await;

    tokio::time::advance(std::time::Duration::from_secs(10)).await;
    tokio::task::yield_now().await;

    assert_eq!(
        *responder
            .edits
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
        vec![(20, 200)]
    );
}

#[tokio::test(start_paused = true)]
async fn unchanged_paused_panel_is_not_edited() {
    let responder = Arc::new(RecordingResponder::default());
    let updater = NowPanelUpdater::new(
        responder.clone(),
        ComponentIdCodec::new([7; 32]).expect("component codec"),
        Arc::from([]),
    );
    let mut paused = snapshot(3_000);
    paused.state = PlayerState::Paused;
    updater.track_source(
        paused.guild_id,
        ChannelId::new(20),
        MessageId::new(200),
        Arc::new(FixedSource(paused.clone())),
        &paused,
    );
    tokio::task::yield_now().await;

    tokio::time::advance(std::time::Duration::from_secs(20)).await;
    tokio::task::yield_now().await;

    assert!(
        responder
            .edits
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_empty()
    );
}

fn snapshot(position_ms: u64) -> PlayerSnapshot {
    PlayerSnapshot {
        guild_id: GuildId::new(1).expect("guild"),
        voice_channel_id: None,
        revision: StateRevision::new(1),
        state: PlayerState::Playing,
        current_track: Some(TrackSnapshot {
            track_id: Uuid::from_u128(1),
            title: "Example".to_owned(),
            artist: None,
            album: None,
            provenance: None,
            requester_user_id: None,
            duration_ms: Some(120_000),
            position_ms,
            seekable: true,
        }),
        queued_tracks: 0,
        upcoming_tracks: Vec::new(),
        has_previous_track: false,
        volume: Volume::DEFAULT,
        repeat_mode: RepeatMode::Off,
        shuffle_enabled: false,
        hrir_preset: None,
        spatial_audio_enabled: false,
        observed_at: UnixTimeMillis::new(0),
    }
}
