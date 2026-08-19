use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use pepeaudio_core::PlayerState;
use pepeaudio_player::PlayerEvent;
use uuid::Uuid;

pub(super) fn upcoming_ids(snapshot: &pepeaudio_core::PlayerSnapshot) -> Vec<Uuid> {
    snapshot
        .upcoming_tracks
        .iter()
        .map(|track| track.track_id)
        .collect()
}

pub(super) struct DropMarker(pub(super) Arc<AtomicUsize>);

impl Drop for DropMarker {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

pub(super) async fn wait_for_shutdown_snapshot(
    events: &mut tokio::sync::broadcast::Receiver<PlayerEvent>,
) -> pepeaudio_core::PlayerSnapshot {
    let mut terminal = None;
    loop {
        match events.recv().await.expect("event channel remains open") {
            PlayerEvent::StateChanged(snapshot) if snapshot.state == PlayerState::Disconnected => {
                terminal = Some(snapshot);
            }
            PlayerEvent::Shutdown(_) => {
                return *terminal.expect("terminal snapshot precedes shutdown");
            }
            _ => {}
        }
    }
}
