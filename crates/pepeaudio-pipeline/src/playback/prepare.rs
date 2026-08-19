use std::{sync::Arc, time::Duration};

use pepeaudio_player::{PlaybackGeneration, QueueTrack};
use songbird::{Call, tracks::Track};
use tokio::{sync::Mutex, task::JoinHandle};

use super::SongbirdPlayback;
use crate::{
    PipelineError, PipelineResult,
    cancellation::Cancellation,
    decoder::{DecoderProcessSlot, DecoderReplacementPermit},
    dsp::DspController,
    pcm::spawn_pcm_worker,
    resolver::ResolvedSource,
    track::{ActiveTrack, ActiveTrackParts, TrackLifecycle, attach_events},
    worker_cleanup::WorkerCleanup,
};

pub(super) struct PreparedTrack {
    queue_track: QueueTrack,
    source: ResolvedSource,
    generation: PlaybackGeneration,
    base_position: Duration,
    paused: bool,
    input: Option<songbird::input::Input>,
    dsp: DspController,
    lifecycle: Arc<TrackLifecycle>,
    process_slot: DecoderProcessSlot,
    replacement_permit: Option<DecoderReplacementPermit>,
    worker: Option<JoinHandle<()>>,
}

impl PreparedTrack {
    async fn install(mut self, call: &Arc<Mutex<Call>>) -> ActiveTrack {
        debug_assert!(self.replacement_permit.is_none());
        let input = self
            .input
            .take()
            .expect("a prepared PCM input is installed exactly once");
        let mut songbird_track = Track::new_with_uuid(input, self.queue_track.track_id).volume(1.0);
        if self.paused {
            songbird_track = songbird_track.pause();
        }
        attach_events(&mut songbird_track, &self.lifecycle);
        let handle = call.lock().await.play_only(songbird_track);
        ActiveTrack::new(ActiveTrackParts {
            queue_track: self.queue_track.clone(),
            source: self.source.clone(),
            generation: self.generation,
            base_position: self.base_position,
            paused: self.paused,
            handle,
            dsp: self.dsp.clone(),
            lifecycle: Arc::clone(&self.lifecycle),
            process_slot: self.process_slot.clone(),
            worker: self
                .worker
                .take()
                .expect("a prepared PCM worker is installed exactly once"),
        })
    }

    fn release_replacement_permit(&mut self) {
        if let Some(replacement_permit) = self.replacement_permit.take() {
            replacement_permit.release();
        }
    }
}

impl Drop for PreparedTrack {
    fn drop(&mut self) {
        if let Some(worker) = &self.worker {
            self.lifecycle.mark_manual();
            worker.abort();
        }
    }
}

impl SongbirdPlayback {
    pub(super) async fn prepare_track(
        &mut self,
        track: &QueueTrack,
        source: ResolvedSource,
        start: Duration,
        paused: bool,
        generation: PlaybackGeneration,
        replacement_slot: Option<&DecoderProcessSlot>,
    ) -> PipelineResult<PreparedTrack> {
        let spawned = if let Some(active_slot) = replacement_slot {
            self.dependencies
                .decoder
                .spawn_replacement(&source, start, active_slot)
                .await?
        } else {
            self.dependencies.decoder.spawn(&source, start).await?
        };
        let (decoder, process_slot, replacement_permit) = spawned.into_parts();
        let cancellation = Cancellation::default();
        let lifecycle = TrackLifecycle::new(
            self.guild_id,
            track.track_id,
            generation,
            Arc::clone(&self.current_generation),
            cancellation,
            self.events.clone(),
        );
        let worker = spawn_pcm_worker(
            decoder,
            process_slot.clone(),
            replacement_permit.clone(),
            self.state.clone(),
            self.config,
            start,
            Arc::clone(&lifecycle),
        )
        .await?;
        Ok(PreparedTrack {
            queue_track: track.clone(),
            source,
            generation,
            base_position: start,
            paused,
            input: Some(worker.input),
            dsp: worker.controller,
            lifecycle,
            process_slot,
            replacement_permit,
            worker: Some(worker.task),
        })
    }

    pub(super) async fn replace_active(
        &mut self,
        mut prepared: PreparedTrack,
    ) -> PipelineResult<()> {
        let call = Arc::clone(self.call.as_ref().ok_or(PipelineError::NotConnected)?);
        self.publish_generation(prepared.generation);
        if let Some(active) = self.active.take() {
            let track_id = active.queue_track.track_id;
            let cleanup = active.shutdown(self.config.shutdown_timeout).await;
            report_cleanup(self.guild_id.get(), track_id, cleanup);
        }
        prepared.release_replacement_permit();
        self.active = Some(prepared.install(&call).await);
        Ok(())
    }

    pub(super) async fn stop_active(&mut self) -> PipelineResult<()> {
        let Some(active) = self.active.take() else {
            return Ok(());
        };
        self.invalidate_generation();
        let track_id = active.queue_track.track_id;
        let cleanup = active.shutdown(self.config.shutdown_timeout).await;
        report_cleanup(self.guild_id.get(), track_id, cleanup);
        Ok(())
    }
}

fn report_cleanup(guild_id: u64, track_id: uuid::Uuid, cleanup: WorkerCleanup) {
    match cleanup {
        WorkerCleanup::Finished => {}
        WorkerCleanup::Failed => tracing::warn!(
            guild_id,
            %track_id,
            "playback worker ended unexpectedly during cleanup"
        ),
        WorkerCleanup::AbortedAfterTimeout => tracing::warn!(
            guild_id,
            %track_id,
            "playback worker was aborted after its cleanup deadline"
        ),
    }
}

#[cfg(test)]
mod tests;
