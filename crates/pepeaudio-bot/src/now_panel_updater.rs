use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard},
    time::Duration,
};

use async_trait::async_trait;
use pepeaudio_core::{GuildId, PlayerSnapshot, PlayerState, StateRevision};
use pepeaudio_player::PlayerHandle;
use serenity::model::id::{ChannelId, MessageId};
use tokio::{
    task::AbortHandle,
    time::{Instant, MissedTickBehavior, interval_at},
};

use crate::{ComponentIdCodec, ComponentsV2Responder, HrirOption, build_now_panel};

const UPDATE_INTERVAL: Duration = Duration::from_secs(10);
const SNAPSHOT_TIMEOUT: Duration = Duration::from_secs(3);

/// Keeps only the newest `/now` panel for each guild synchronized with playback.
#[derive(Clone)]
pub struct NowPanelUpdater {
    inner: Arc<UpdaterInner>,
}

struct UpdaterInner {
    components: Arc<dyn ComponentsV2Responder>,
    component_ids: ComponentIdCodec,
    hrir_options: Arc<[HrirOption]>,
    registrations: Mutex<HashMap<GuildId, Registration>>,
}

struct Registration {
    abort: AbortHandle,
}

struct UpdateContext {
    guild_id: GuildId,
    channel_id: ChannelId,
    message_id: MessageId,
    source: Arc<dyn SnapshotSource>,
    components: Arc<dyn ComponentsV2Responder>,
    component_ids: ComponentIdCodec,
    hrir_options: Arc<[HrirOption]>,
}

impl Drop for Registration {
    fn drop(&mut self) {
        self.abort.abort();
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct RenderKey {
    revision: StateRevision,
    position_seconds: u64,
}

#[async_trait]
trait SnapshotSource: Send + Sync + 'static {
    async fn snapshot(&self) -> Option<PlayerSnapshot>;
}

struct PlayerSnapshotSource(PlayerHandle);

#[async_trait]
impl SnapshotSource for PlayerSnapshotSource {
    async fn snapshot(&self) -> Option<PlayerSnapshot> {
        tokio::time::timeout(SNAPSHOT_TIMEOUT, self.0.snapshot())
            .await
            .ok()?
            .ok()
    }
}

impl NowPanelUpdater {
    #[must_use]
    pub fn new(
        components: Arc<dyn ComponentsV2Responder>,
        component_ids: ComponentIdCodec,
        hrir_options: Arc<[HrirOption]>,
    ) -> Self {
        Self {
            inner: Arc::new(UpdaterInner {
                components,
                component_ids,
                hrir_options,
                registrations: Mutex::new(HashMap::new()),
            }),
        }
    }

    pub(crate) fn track(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
        message_id: MessageId,
        player: PlayerHandle,
        initial_snapshot: &PlayerSnapshot,
    ) {
        self.track_source(
            guild_id,
            channel_id,
            message_id,
            Arc::new(PlayerSnapshotSource(player)),
            initial_snapshot,
        );
    }

    fn track_source(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
        message_id: MessageId,
        source: Arc<dyn SnapshotSource>,
        initial_snapshot: &PlayerSnapshot,
    ) {
        let components = self.inner.components.clone();
        let component_ids = self.inner.component_ids.clone();
        let hrir_options = self.inner.hrir_options.clone();
        let initial_key = render_key(initial_snapshot);
        let task = tokio::spawn(update_loop(
            UpdateContext {
                guild_id,
                channel_id,
                message_id,
                source,
                components,
                component_ids,
                hrir_options,
            },
            initial_key,
        ));
        let registration = Registration {
            abort: task.abort_handle(),
        };
        drop(task);
        registrations(&self.inner).insert(guild_id, registration);
    }
}

async fn update_loop(context: UpdateContext, mut last_key: RenderKey) {
    let mut ticks = interval_at(Instant::now() + UPDATE_INTERVAL, UPDATE_INTERVAL);
    ticks.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        ticks.tick().await;
        let Some(snapshot) = context.source.snapshot().await else {
            tracing::warn!(
                guild_id = context.guild_id.get(),
                "latest now panel snapshot failed"
            );
            return;
        };
        let next_key = render_key(&snapshot);
        if next_key == last_key {
            continue;
        }
        let Ok(panel) = build_now_panel(&snapshot, &context.component_ids, &context.hrir_options)
        else {
            tracing::warn!(
                guild_id = context.guild_id.get(),
                "latest now panel rendering failed"
            );
            return;
        };
        if context
            .components
            .edit_message(context.channel_id, context.message_id, &panel)
            .await
            .is_err()
        {
            tracing::warn!(
                guild_id = context.guild_id.get(),
                channel_id = context.channel_id.get(),
                message_id = context.message_id.get(),
                "latest now panel update failed"
            );
            return;
        }
        last_key = next_key;
    }
}

fn render_key(snapshot: &PlayerSnapshot) -> RenderKey {
    let position_seconds = if snapshot.state == PlayerState::Playing {
        snapshot
            .current_track
            .as_ref()
            .map_or(0, |track| track.position_ms / 1_000)
    } else {
        0
    };
    RenderKey {
        revision: snapshot.revision,
        position_seconds,
    }
}

fn registrations(inner: &UpdaterInner) -> MutexGuard<'_, HashMap<GuildId, Registration>> {
    inner
        .registrations
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
#[path = "now_panel_updater_tests.rs"]
mod tests;
