use std::{
    future::pending,
    sync::{Arc, atomic::AtomicU64},
    time::Duration,
};

use pepeaudio_player::{PlaybackGeneration, PlaybackSource, QueueTrack};
use serenity::model::id::{GuildId as SerenityGuildId, UserId as SerenityUserId};
use songbird::Call;
use tokio::sync::{Mutex, broadcast, mpsc};

use super::PreparedTrack;
use crate::{
    cancellation::Cancellation, decoder::DecoderProcessSlot, dsp::DspController,
    resolver::ResolvedSource, songbird_input::songbird_pcm_input, track::TrackLifecycle,
};

#[tokio::test]
async fn installing_prepared_track_keeps_its_lifecycle_running() {
    let generation = PlaybackGeneration::new(7);
    let track = QueueTrack::new(
        "test track",
        None,
        Some(1_000),
        true,
        PlaybackSource::new("test.raw"),
    );
    let current_generation = Arc::new(AtomicU64::new(generation.get()));
    let (events, _) = broadcast::channel(4);
    let lifecycle = TrackLifecycle::new(
        pepeaudio_core::GuildId::new(1).expect("guild ID"),
        track.track_id,
        generation,
        current_generation,
        Cancellation::default(),
        events,
    );
    let (reader, _writer) = tokio::io::duplex(64);
    let (dsp_sender, _dsp_receiver) = mpsc::channel(1);
    let worker = tokio::spawn(pending::<()>());
    let prepared = PreparedTrack {
        queue_track: track,
        source: ResolvedSource::new("test.raw"),
        generation,
        base_position: Duration::ZERO,
        paused: false,
        input: Some(songbird_pcm_input(reader, 64)),
        dsp: DspController::new(dsp_sender),
        lifecycle: Arc::clone(&lifecycle),
        process_slot: DecoderProcessSlot::untracked(),
        replacement_permit: None,
        worker: Some(worker),
    };
    let call = Arc::new(Mutex::new(Call::standalone(
        SerenityGuildId::new(1),
        SerenityUserId::new(2),
    )));

    let active = prepared.install(&call).await;

    assert!(active.lifecycle.accepts_dsp_control());
    drop(active);
}
