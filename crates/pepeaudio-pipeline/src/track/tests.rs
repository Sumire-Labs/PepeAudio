use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use pepeaudio_core::GuildId;
use pepeaudio_player::PlaybackGeneration;
use tokio::sync::broadcast;

use super::TrackLifecycle;
use crate::{PlaybackEndReason, PlaybackEvent, WorkerFailure, cancellation::Cancellation};

#[tokio::test]
async fn worker_failure_is_classified_once_before_end() {
    let (lifecycle, mut events, _) = fixture(7);
    lifecycle.report_worker_failure(WorkerFailure::Decoder);
    lifecycle.songbird_end();

    assert!(matches!(
        events.recv().await.expect("worker event"),
        PlaybackEvent::WorkerFailed {
            identity,
            failure: WorkerFailure::Decoder,
            ..
        } if identity.generation() == PlaybackGeneration::new(7)
    ));
    assert!(matches!(
        events.recv().await.expect("end event"),
        PlaybackEvent::TrackEnded {
            identity,
            reason: PlaybackEndReason::WorkerFailed,
            ..
        } if identity.generation() == PlaybackGeneration::new(7)
    ));
    lifecycle.songbird_end();
    assert!(events.try_recv().is_err());
}

#[test]
fn manual_and_stale_ends_are_suppressed() {
    let (manual, mut manual_events, _) = fixture(3);
    manual.mark_manual();
    manual.songbird_end();
    assert!(manual_events.try_recv().is_err());

    let (stale, mut stale_events, generation) = fixture(4);
    generation.store(5, Ordering::Release);
    stale.report_worker_failure(WorkerFailure::Audio);
    stale.songbird_end();
    assert!(stale_events.try_recv().is_err());
}

#[tokio::test]
async fn songbird_error_wins_end_classification() {
    let (lifecycle, mut events, _) = fixture(11);
    lifecycle.songbird_error();
    lifecycle.songbird_end();
    assert!(matches!(
        events.recv().await.expect("end event"),
        PlaybackEvent::TrackEnded {
            reason: PlaybackEndReason::SongbirdError,
            ..
        }
    ));
}

fn fixture(
    active_generation: u64,
) -> (
    Arc<TrackLifecycle>,
    broadcast::Receiver<PlaybackEvent>,
    Arc<AtomicU64>,
) {
    let (sender, receiver) = broadcast::channel(8);
    let generation = Arc::new(AtomicU64::new(active_generation));
    let lifecycle = TrackLifecycle::new(
        GuildId::new(1).expect("guild"),
        uuid::Uuid::new_v4(),
        PlaybackGeneration::new(active_generation),
        Arc::clone(&generation),
        Cancellation::default(),
        sender,
    );
    (lifecycle, receiver, generation)
}
