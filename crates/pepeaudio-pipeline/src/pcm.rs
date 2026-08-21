use std::sync::Arc;

use pepeaudio_audio::AudioProcessor;
use songbird::input::Input;
use tokio::{
    io::{AsyncWriteExt, DuplexStream},
    sync::mpsc,
    task::JoinHandle,
};

use crate::{
    DecodedPcm, PipelineConfig, PipelineError, PipelineResult,
    decoder::{DecoderProcessSlot, DecoderReplacementPermit},
    dsp::{DspCommand, DspController, DspState, apply_command},
    event::WorkerFailure,
    orbit::SpatialPosition,
    songbird_input::songbird_pcm_input,
    track::TrackLifecycle,
};

const PCM_FRAME_BYTES: usize = 2 * size_of::<f32>();

pub(crate) struct PcmWorker {
    pub(crate) input: Input,
    pub(crate) controller: DspController,
    pub(crate) task: JoinHandle<()>,
}

pub(crate) async fn spawn_pcm_worker(
    mut decoder: Box<dyn DecodedPcm>,
    process_slot: DecoderProcessSlot,
    replacement_permit: Option<DecoderReplacementPermit>,
    state: DspState,
    config: PipelineConfig,
    lifecycle: Arc<TrackLifecycle>,
) -> PipelineResult<PcmWorker> {
    let orbit_origin = state.orbit_origin;
    let processor_result =
        tokio::task::spawn_blocking(move || state.build_processor(config.block_frames)).await;
    let processor = match processor_result {
        Ok(Ok(processor)) => processor,
        Ok(Err(error)) => {
            let _ = decoder.shutdown().await;
            return Err(error);
        }
        Err(_) => {
            let _ = decoder.shutdown().await;
            return Err(PipelineError::WorkerTask);
        }
    };
    let (reader, writer) = tokio::io::duplex(config.pcm_buffer_bytes);
    let input = songbird_pcm_input(reader, config.songbird_buffer_bytes);
    let (sender, receiver) = mpsc::channel(config.control_capacity);
    let controller = DspController::new(sender);
    let spatial_position = SpatialPosition::new(orbit_origin);
    let task = tokio::spawn(async move {
        let _process_slot = process_slot;
        let _replacement_permit = replacement_permit;
        run_worker(
            decoder,
            processor,
            spatial_position,
            writer,
            receiver,
            config,
            lifecycle,
        )
        .await;
    });
    Ok(PcmWorker {
        input,
        controller,
        task,
    })
}

async fn run_worker(
    mut decoder: Box<dyn DecodedPcm>,
    processor: AudioProcessor,
    spatial_position: SpatialPosition,
    writer: DuplexStream,
    controls: mpsc::Receiver<DspCommand>,
    config: PipelineConfig,
    lifecycle: Arc<TrackLifecycle>,
) {
    let result = pump_pcm(
        decoder.as_mut(),
        processor,
        spatial_position,
        writer,
        controls,
        config,
        &lifecycle,
    )
    .await;
    match result {
        Ok(WorkerCompletion::Natural) => lifecycle.mark_natural(),
        Ok(WorkerCompletion::Cancelled) => {
            let _ = decoder.shutdown().await;
        }
        Err(error) => {
            let _ = decoder.shutdown().await;
            if !lifecycle.cancellation().is_cancelled() {
                lifecycle.report_worker_failure(classify_failure(&error));
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WorkerCompletion {
    Natural,
    Cancelled,
}

async fn pump_pcm(
    decoder: &mut dyn DecodedPcm,
    processor: AudioProcessor,
    mut spatial_position: SpatialPosition,
    mut writer: DuplexStream,
    mut controls: mpsc::Receiver<DspCommand>,
    config: PipelineConfig,
    lifecycle: &TrackLifecycle,
) -> PipelineResult<WorkerCompletion> {
    let sample_capacity = config
        .block_frames
        .checked_mul(2)
        .ok_or(PipelineError::InvalidConfig)?;
    let byte_capacity = sample_capacity
        .checked_mul(size_of::<f32>())
        .ok_or(PipelineError::InvalidConfig)?;
    let mut bytes = vec![0_u8; byte_capacity];
    let mut input = vec![0.0_f32; sample_capacity];
    let mut output = vec![0.0_f32; sample_capacity];
    let mut encoded = vec![0_u8; byte_capacity];
    let mut processor = Some(processor);
    let mut filled = 0_usize;
    let mut controls_open = true;

    loop {
        let read = loop {
            tokio::select! {
                biased;
                () = lifecycle.cancellation().cancelled() => {
                    return Ok(WorkerCompletion::Cancelled);
                }
                command = controls.recv(), if controls_open => {
                    if let Some(command) = command {
                        apply_command(
                            &mut processor,
                            &mut spatial_position,
                            command,
                            config.transition_frames,
                        );
                    } else {
                        controls_open = false;
                    }
                }
                result = decoder.read_pcm(&mut bytes[filled..]) => break result?,
            }
        };
        if read == 0 {
            if filled != 0 {
                return Err(PipelineError::PartialPcmFrame);
            }
            tokio::select! {
                biased;
                () = lifecycle.cancellation().cancelled() => {
                    return Ok(WorkerCompletion::Cancelled);
                }
                result = decoder.finish() => result?,
            }
            writer
                .shutdown()
                .await
                .map_err(PipelineError::DecoderPipe)?;
            return Ok(WorkerCompletion::Natural);
        }
        filled = filled
            .checked_add(read)
            .ok_or(PipelineError::InvalidConfig)?;
        let complete_bytes = filled - (filled % PCM_FRAME_BYTES);
        if complete_bytes == 0 {
            continue;
        }
        let samples = complete_bytes / size_of::<f32>();
        decode_samples(&bytes[..complete_bytes], &mut input[..samples]);
        {
            let active_processor = processor.as_mut().ok_or(PipelineError::WorkerClosed)?;
            active_processor.set_orbit_position(spatial_position.position())?;
            active_processor.process_block(&input[..samples], &mut output[..samples])?;
        }
        encode_samples(&output[..samples], &mut encoded[..complete_bytes]);
        write_processed(
            &mut writer,
            &encoded[..complete_bytes],
            &mut controls,
            &mut controls_open,
            &mut processor,
            &mut spatial_position,
            config.transition_frames,
            lifecycle,
        )
        .await?;
        bytes.copy_within(complete_bytes..filled, 0);
        filled -= complete_bytes;
    }
}

fn decode_samples(bytes: &[u8], samples: &mut [f32]) {
    for (chunk, sample) in bytes.chunks_exact(size_of::<f32>()).zip(samples) {
        *sample = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
    }
}

fn encode_samples(samples: &[f32], bytes: &mut [u8]) {
    for (sample, chunk) in samples.iter().zip(bytes.chunks_exact_mut(size_of::<f32>())) {
        chunk.copy_from_slice(&sample.to_le_bytes());
    }
}

#[allow(clippy::too_many_arguments)]
async fn write_processed(
    writer: &mut DuplexStream,
    bytes: &[u8],
    controls: &mut mpsc::Receiver<DspCommand>,
    controls_open: &mut bool,
    processor: &mut Option<AudioProcessor>,
    spatial_position: &mut SpatialPosition,
    transition_frames: usize,
    lifecycle: &TrackLifecycle,
) -> PipelineResult<()> {
    let mut written = 0_usize;
    while written < bytes.len() {
        tokio::select! {
            biased;
            () = lifecycle.cancellation().cancelled() => return Err(PipelineError::WorkerClosed),
            command = controls.recv(), if *controls_open => {
                if let Some(command) = command {
                    apply_command(processor, spatial_position, command, transition_frames);
                } else {
                    *controls_open = false;
                }
            }
            result = writer.write(&bytes[written..]) => {
                let count = result.map_err(|_| PipelineError::OutputClosed)?;
                if count == 0 {
                    return Err(PipelineError::OutputClosed);
                }
                written = written.checked_add(count).ok_or(PipelineError::InvalidConfig)?;
            }
        }
    }
    Ok(())
}

fn classify_failure(error: &PipelineError) -> WorkerFailure {
    match error {
        PipelineError::PartialPcmFrame | PipelineError::Dsp(_) => WorkerFailure::Audio,
        PipelineError::OutputClosed => WorkerFailure::Output,
        PipelineError::DecoderSpawn(_)
        | PipelineError::DecoderPipe(_)
        | PipelineError::DecoderLifecycle(_)
        | PipelineError::DecoderExit { .. }
        | PipelineError::DecoderDiagnosticsTooLarge => WorkerFailure::Decoder,
        _ => WorkerFailure::Task,
    }
}

#[cfg(test)]
mod tests;
