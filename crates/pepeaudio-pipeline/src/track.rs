use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering},
};

use async_trait::async_trait;
use pepeaudio_core::GuildId;
use pepeaudio_player::{PlaybackGeneration, PlaybackIdentity, QueueTrack};
use songbird::{
    Event, EventContext, EventHandler, TrackEvent,
    events::EventData,
    tracks::{Track, TrackHandle},
};
use tokio::{sync::broadcast, task::JoinHandle};

use crate::{
    cancellation::Cancellation,
    decoder::DecoderProcessSlot,
    dsp::DspController,
    event::{PlaybackEndReason, PlaybackEvent, WorkerFailure},
    resolver::ResolvedSource,
    worker_cleanup::{WorkerCleanup, finish_worker},
};

const RUNNING: u8 = 0;
const NATURAL: u8 = 1;
const FAILED: u8 = 2;
const SONGBIRD_ERROR: u8 = 3;
const SONGBIRD_ENDED: u8 = 4;
const MANUAL: u8 = 5;

#[derive(Debug)]
pub(crate) struct TrackLifecycle {
    guild_id: GuildId,
    identity: PlaybackIdentity,
    current_generation: Arc<AtomicU64>,
    outcome: AtomicU8,
    end_emitted: AtomicBool,
    cancellation: Cancellation,
    events: broadcast::Sender<PlaybackEvent>,
}

impl TrackLifecycle {
    pub(crate) fn new(
        guild_id: GuildId,
        track_id: uuid::Uuid,
        generation: PlaybackGeneration,
        current_generation: Arc<AtomicU64>,
        cancellation: Cancellation,
        events: broadcast::Sender<PlaybackEvent>,
    ) -> Arc<Self> {
        Arc::new(Self {
            guild_id,
            identity: PlaybackIdentity::new(track_id, generation),
            current_generation,
            outcome: AtomicU8::new(RUNNING),
            end_emitted: AtomicBool::new(false),
            cancellation,
            events,
        })
    }

    pub(crate) fn cancellation(&self) -> &Cancellation {
        &self.cancellation
    }

    pub(crate) fn mark_natural(&self) {
        let _ =
            self.outcome
                .compare_exchange(RUNNING, NATURAL, Ordering::AcqRel, Ordering::Acquire);
    }

    pub(crate) fn mark_manual(&self) {
        self.outcome.store(MANUAL, Ordering::Release);
        self.cancellation.cancel();
    }

    pub(crate) fn accepts_dsp_control(&self) -> bool {
        self.outcome.load(Ordering::Acquire) == RUNNING && !self.cancellation.is_cancelled()
    }

    pub(crate) fn report_worker_failure(&self, failure: WorkerFailure) {
        if self
            .outcome
            .compare_exchange(RUNNING, FAILED, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
            && self.is_current()
        {
            let _ = self.events.send(PlaybackEvent::WorkerFailed {
                guild_id: self.guild_id,
                identity: self.identity,
                failure,
            });
        }
    }

    fn songbird_error(&self) {
        if self.is_current() {
            let _ = self.outcome.compare_exchange(
                RUNNING,
                SONGBIRD_ERROR,
                Ordering::AcqRel,
                Ordering::Acquire,
            );
        }
    }

    fn songbird_end(&self) {
        if !self.is_current() || self.outcome.load(Ordering::Acquire) == MANUAL {
            self.cancellation.cancel();
            return;
        }
        let _ = self.outcome.compare_exchange(
            RUNNING,
            SONGBIRD_ENDED,
            Ordering::AcqRel,
            Ordering::Acquire,
        );
        if !self.end_emitted.swap(true, Ordering::AcqRel) {
            let reason = match self.outcome.load(Ordering::Acquire) {
                NATURAL => PlaybackEndReason::Natural,
                FAILED => PlaybackEndReason::WorkerFailed,
                SONGBIRD_ERROR => PlaybackEndReason::SongbirdError,
                _ => PlaybackEndReason::SongbirdEnded,
            };
            let _ = self.events.send(PlaybackEvent::TrackEnded {
                guild_id: self.guild_id,
                identity: self.identity,
                reason,
            });
        }
        self.cancellation.cancel();
    }

    fn is_current(&self) -> bool {
        self.current_generation.load(Ordering::Acquire) == self.identity.generation().get()
    }
}

#[derive(Clone, Copy)]
enum SongbirdSignal {
    End,
    Error,
}

struct SongbirdEventHandler {
    lifecycle: Arc<TrackLifecycle>,
    signal: SongbirdSignal,
}

#[async_trait]
impl EventHandler for SongbirdEventHandler {
    async fn act(&self, _: &EventContext<'_>) -> Option<Event> {
        match self.signal {
            SongbirdSignal::End => self.lifecycle.songbird_end(),
            SongbirdSignal::Error => self.lifecycle.songbird_error(),
        }
        Some(Event::Cancel)
    }
}

pub(crate) fn attach_events(track: &mut Track, lifecycle: &Arc<TrackLifecycle>) {
    for (event, signal) in [
        (TrackEvent::End, SongbirdSignal::End),
        (TrackEvent::Error, SongbirdSignal::Error),
    ] {
        track.events.add_event(
            EventData::new(
                Event::Track(event),
                SongbirdEventHandler {
                    lifecycle: Arc::clone(lifecycle),
                    signal,
                },
            ),
            std::time::Duration::ZERO,
        );
    }
}

pub(crate) struct ActiveTrack {
    pub(crate) queue_track: QueueTrack,
    pub(crate) source: ResolvedSource,
    pub(crate) generation: PlaybackGeneration,
    pub(crate) base_position: std::time::Duration,
    pub(crate) paused: bool,
    pub(crate) handle: TrackHandle,
    pub(crate) dsp: DspController,
    pub(crate) lifecycle: Arc<TrackLifecycle>,
    pub(crate) process_slot: DecoderProcessSlot,
    worker: Option<JoinHandle<()>>,
    shutdown_started: bool,
}

pub(crate) struct ActiveTrackParts {
    pub(crate) queue_track: QueueTrack,
    pub(crate) source: ResolvedSource,
    pub(crate) generation: PlaybackGeneration,
    pub(crate) base_position: std::time::Duration,
    pub(crate) paused: bool,
    pub(crate) handle: TrackHandle,
    pub(crate) dsp: DspController,
    pub(crate) lifecycle: Arc<TrackLifecycle>,
    pub(crate) process_slot: DecoderProcessSlot,
    pub(crate) worker: JoinHandle<()>,
}

impl ActiveTrack {
    pub(crate) fn new(parts: ActiveTrackParts) -> Self {
        Self {
            queue_track: parts.queue_track,
            source: parts.source,
            generation: parts.generation,
            base_position: parts.base_position,
            paused: parts.paused,
            handle: parts.handle,
            dsp: parts.dsp,
            lifecycle: parts.lifecycle,
            process_slot: parts.process_slot,
            worker: Some(parts.worker),
            shutdown_started: false,
        }
    }

    pub(crate) async fn shutdown(mut self, wait: std::time::Duration) -> WorkerCleanup {
        self.begin_shutdown();
        let Some(worker) = self.worker.take() else {
            return WorkerCleanup::Finished;
        };
        finish_worker(worker, wait).await
    }

    fn begin_shutdown(&mut self) {
        if self.shutdown_started {
            return;
        }
        self.shutdown_started = true;
        self.lifecycle.mark_manual();
        // A closed handle means Songbird has already discarded the track.
        let _ = self.handle.stop();
    }
}

impl Drop for ActiveTrack {
    fn drop(&mut self) {
        self.begin_shutdown();
        if let Some(worker) = &self.worker {
            worker.abort();
        }
    }
}

#[cfg(test)]
mod tests;
