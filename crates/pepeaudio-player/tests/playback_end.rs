mod common;

use std::{collections::HashSet, sync::Arc, time::Duration};

use async_trait::async_trait;
use pepeaudio_core::{ChannelId, HrirPresetId, PlayerCommand, PlayerState, RepeatMode, Volume};
use pepeaudio_player::{
    NoopSnapshotPublisher, PlaybackEndReason, PlaybackGeneration, PlaybackIdentity, PlaybackPort,
    PlayerConfig, PlayerEvent, QueueTrack, spawn_player,
};
use thiserror::Error;
use tokio::sync::Mutex;
use uuid::Uuid;

use common::{command, connect, guild, revision, track};

#[derive(Clone, Default)]
struct ControlledPlayback {
    calls: Arc<Mutex<Vec<Uuid>>>,
    failing: Arc<Mutex<HashSet<Uuid>>>,
}

impl ControlledPlayback {
    async fn fail(&self, tracks: impl IntoIterator<Item = Uuid>) {
        self.failing.lock().await.extend(tracks);
    }

    async fn calls(&self) -> Vec<Uuid> {
        self.calls.lock().await.clone()
    }
}

#[derive(Debug, Error)]
#[error("selected track failed")]
struct SelectedTrackFailed;

#[async_trait]
impl PlaybackPort for ControlledPlayback {
    type Error = SelectedTrackFailed;

    async fn connect(&mut self, _: ChannelId) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn play(&mut self, track: &QueueTrack, _: PlaybackGeneration) -> Result<(), Self::Error> {
        self.calls.lock().await.push(track.track_id);
        if self.failing.lock().await.contains(&track.track_id) {
            Err(SelectedTrackFailed)
        } else {
            Ok(())
        }
    }

    async fn pause(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn resume(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn seek(&mut self, _: u64, _: PlaybackGeneration) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn set_volume(&mut self, _: Volume) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn set_hrir(&mut self, _: &HrirPresetId) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn set_spatial_audio(&mut self, _: bool) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

fn harness() -> (
    pepeaudio_player::PlayerRuntime,
    pepeaudio_player::PlayerHandle,
    ControlledPlayback,
) {
    let playback = ControlledPlayback::default();
    let config = PlayerConfig::new(16, 64, 8, Duration::from_mins(5)).expect("config");
    let runtime = spawn_player(guild(), config, playback.clone(), NoopSnapshotPublisher);
    let handle = runtime.handle();
    (runtime, handle, playback)
}

#[tokio::test]
async fn failed_end_never_repeats_the_failed_current_track() {
    let (runtime, handle, playback) = harness();
    connect(&handle).await;
    let current = track("failed-current");
    let next = track("next");
    let current_id = current.track_id;
    let next_id = next.track_id;
    handle.enqueue(current, revision(1)).await.expect("current");
    handle.enqueue(next, revision(2)).await.expect("next");
    handle
        .apply(command(
            3,
            PlayerCommand::SetRepeat {
                mode: RepeatMode::Track,
            },
        ))
        .await
        .expect("repeat track");

    let snapshot = handle
        .playback_ended(
            PlaybackIdentity::new(current_id, PlaybackGeneration::new(1)),
            PlaybackEndReason::WorkerFailed,
        )
        .await
        .expect("failed end is committed");

    assert_eq!(
        snapshot.current_track.as_ref().map(|item| item.track_id),
        Some(next_id)
    );
    assert_eq!(playback.calls().await, vec![current_id, next_id]);
    runtime.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn natural_repeat_failure_advances_through_each_candidate_once() {
    let (runtime, handle, playback) = harness();
    connect(&handle).await;
    let current = track("current");
    let first = track("first-broken");
    let second = track("second-broken");
    let last = track("last-good");
    let ids = [
        current.track_id,
        first.track_id,
        second.track_id,
        last.track_id,
    ];
    handle.enqueue(current, revision(1)).await.expect("current");
    handle.enqueue(first, revision(2)).await.expect("first");
    handle.enqueue(second, revision(3)).await.expect("second");
    handle.enqueue(last, revision(4)).await.expect("last");
    handle
        .apply(command(
            5,
            PlayerCommand::SetRepeat {
                mode: RepeatMode::Track,
            },
        ))
        .await
        .expect("repeat track");
    playback.fail(ids[..3].iter().copied()).await;

    let snapshot = handle
        .playback_ended(
            PlaybackIdentity::new(ids[0], PlaybackGeneration::new(1)),
            PlaybackEndReason::Natural,
        )
        .await
        .expect("finite fallback");

    assert_eq!(
        snapshot.current_track.as_ref().map(|item| item.track_id),
        Some(ids[3])
    );
    assert_eq!(snapshot.queued_tracks, 0);
    assert_eq!(
        playback.calls().await,
        vec![ids[0], ids[0], ids[1], ids[2], ids[3]]
    );
    runtime.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn previous_after_repeat_queue_removes_the_wrapped_queue_copy() {
    let (runtime, handle, _) = harness();
    connect(&handle).await;
    let first = track("first");
    let second = track("second");
    let first_id = first.track_id;
    let second_id = second.track_id;
    handle.enqueue(first, revision(1)).await.expect("first");
    handle.enqueue(second, revision(2)).await.expect("second");
    handle
        .apply(command(
            3,
            PlayerCommand::SetRepeat {
                mode: RepeatMode::Queue,
            },
        ))
        .await
        .expect("repeat queue");

    let advanced = handle
        .playback_ended(
            PlaybackIdentity::new(first_id, PlaybackGeneration::new(1)),
            PlaybackEndReason::Natural,
        )
        .await
        .expect("advance and wrap first track");
    assert_eq!(
        advanced.current_track.as_ref().map(|track| track.track_id),
        Some(second_id)
    );
    assert_eq!(
        advanced
            .upcoming_tracks
            .iter()
            .map(|track| track.track_id)
            .collect::<Vec<_>>(),
        vec![first_id]
    );

    let returned = handle
        .apply(command(advanced.revision.get(), PlayerCommand::Previous))
        .await
        .expect("return to wrapped previous track");
    assert_eq!(
        returned.current_track.as_ref().map(|track| track.track_id),
        Some(first_id)
    );
    assert_eq!(
        returned
            .upcoming_tracks
            .iter()
            .map(|track| track.track_id)
            .collect::<Vec<_>>(),
        vec![second_id]
    );
    returned
        .validate_public_shape()
        .expect("unique track identities");
    runtime.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn a_different_track_end_does_not_change_the_current_playback() {
    let (runtime, handle, playback) = harness();
    connect(&handle).await;
    let current = track("current");
    let queued = track("queued");
    let stale_id = Uuid::new_v4();
    let current_id = current.track_id;
    handle.enqueue(current, revision(1)).await.expect("current");
    handle.enqueue(queued, revision(2)).await.expect("queued");
    let before = handle.snapshot().await.expect("before");

    let after = handle
        .playback_ended(
            PlaybackIdentity::new(stale_id, PlaybackGeneration::new(1)),
            PlaybackEndReason::SongbirdError,
        )
        .await
        .expect("stale end is ignored");

    assert_eq!(after.revision, before.revision);
    assert_eq!(
        after.current_track.as_ref().map(|item| item.track_id),
        Some(current_id)
    );
    assert_eq!(after.queued_tracks, 1);
    assert_eq!(playback.calls().await, vec![current_id]);
    runtime.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn delayed_pre_seek_generation_does_not_end_the_sought_playback() {
    let (runtime, handle, playback) = harness();
    connect(&handle).await;
    let current = track("current");
    let queued = track("queued");
    let current_id = current.track_id;
    handle.enqueue(current, revision(1)).await.expect("current");
    handle.enqueue(queued, revision(2)).await.expect("queued");
    let sought = handle
        .apply(command(
            3,
            PlayerCommand::Seek {
                position_ms: 42_000,
            },
        ))
        .await
        .expect("seek creates generation two");

    let after = handle
        .playback_ended(
            PlaybackIdentity::new(current_id, PlaybackGeneration::new(1)),
            PlaybackEndReason::SongbirdEnded,
        )
        .await
        .expect("the delayed generation is ignored");

    assert_eq!(after.revision, sought.revision);
    assert_eq!(
        after.current_track.as_ref().map(|item| item.track_id),
        Some(current_id)
    );
    assert!(
        after
            .current_track
            .as_ref()
            .is_some_and(|item| item.position_ms >= 42_000),
        "the sought playback must remain active"
    );
    assert_eq!(after.queued_tracks, 1);
    assert_eq!(playback.calls().await, vec![current_id]);
    runtime.shutdown().await.expect("shutdown");
}

#[tokio::test(start_paused = true)]
async fn all_failed_candidates_converge_to_idle_then_disconnect() {
    let (runtime, handle, playback) = harness();
    let mut events = handle.subscribe();
    connect(&handle).await;
    let current = track("current");
    let first = track("first-broken");
    let second = track("second-broken");
    let current_id = current.track_id;
    let failed_ids = [first.track_id, second.track_id];
    handle.enqueue(current, revision(1)).await.expect("current");
    handle.enqueue(first, revision(2)).await.expect("first");
    handle.enqueue(second, revision(3)).await.expect("second");
    playback.fail(failed_ids).await;

    let snapshot = handle
        .playback_ended(
            PlaybackIdentity::new(current_id, PlaybackGeneration::new(1)),
            PlaybackEndReason::SongbirdError,
        )
        .await
        .expect("all failures are committed");
    assert_eq!(snapshot.state, PlayerState::IdleConnected);
    assert!(snapshot.current_track.is_none());
    assert_eq!(snapshot.queued_tracks, 0);
    assert_eq!(
        playback.calls().await,
        vec![current_id, failed_ids[0], failed_ids[1]]
    );

    tokio::time::advance(Duration::from_mins(5)).await;
    loop {
        if matches!(
            events.recv().await.expect("event channel remains open"),
            PlayerEvent::IdleDisconnected { .. }
        ) {
            break;
        }
    }
    runtime.shutdown().await.expect("join idle actor");
}
