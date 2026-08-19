use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

use async_trait::async_trait;
use pepeaudio_audio::{AudioProcessor, HorizontalStereoPair, LinearGain, PreparedHrir, RenderMode};
use pepeaudio_core::GuildId;
use pepeaudio_hrir::load_hesuvi_wav;
use pepeaudio_player::PlaybackGeneration;
use tokio::{
    io::AsyncReadExt,
    sync::{broadcast, mpsc},
    time::{Duration, timeout},
};

use super::{pump_pcm, spawn_pcm_worker};
use crate::{
    DecodedPcm, PipelineConfig, PipelineResult,
    cancellation::Cancellation,
    decoder::DecoderProcessSlot,
    dsp::{DspMutation, DspState},
    orbit::OrbitClock,
    track::TrackLifecycle,
};

struct FakeDecoder {
    bytes: Vec<u8>,
    cursor: usize,
    chunk_bytes: usize,
    repeat: bool,
    finished: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
}

#[async_trait]
impl DecodedPcm for FakeDecoder {
    async fn read_pcm(&mut self, output: &mut [u8]) -> PipelineResult<usize> {
        if self.cursor == self.bytes.len() {
            if !self.repeat {
                return Ok(0);
            }
            self.cursor = 0;
        }
        let available = self.bytes.len() - self.cursor;
        let count = available.min(output.len()).min(self.chunk_bytes);
        output[..count].copy_from_slice(&self.bytes[self.cursor..self.cursor + count]);
        self.cursor += count;
        tokio::task::yield_now().await;
        Ok(count)
    }

    async fn finish(&mut self) -> PipelineResult<()> {
        self.finished.store(true, Ordering::Release);
        Ok(())
    }

    async fn shutdown(&mut self) -> PipelineResult<()> {
        self.shutdown.store(true, Ordering::Release);
        Ok(())
    }
}

#[tokio::test]
async fn fragmented_pcm_is_processed_and_finished() {
    let samples = [0.25_f32, -0.5, 0.75, -1.0];
    let bytes = encode(&samples);
    let finished = Arc::new(AtomicBool::new(false));
    let mut decoder = FakeDecoder {
        bytes,
        cursor: 0,
        chunk_bytes: 3,
        repeat: false,
        finished: Arc::clone(&finished),
        shutdown: Arc::new(AtomicBool::new(false)),
    };
    let config = test_config();
    let processor = AudioProcessor::new(
        prepared_identity().as_ref(),
        RenderMode::HorizontalOrbit(HorizontalStereoPair::FRONT),
        config.block_frames,
    )
    .expect("processor");
    let (reader, writer) = tokio::io::duplex(config.pcm_buffer_bytes);
    let (_control_sender, controls) = mpsc::channel(config.control_capacity);
    let lifecycle = lifecycle();
    let mut output = Vec::new();

    let (result, read_result) = tokio::join!(
        pump_pcm(
            &mut decoder,
            processor,
            OrbitClock::new(
                config.orbit_period,
                Duration::ZERO,
                HorizontalStereoPair::FRONT
            )
            .expect("orbit"),
            writer,
            controls,
            config,
            lifecycle.as_ref(),
        ),
        async move {
            let mut reader = reader;
            reader.read_to_end(&mut output).await.map(|_| output)
        }
    );

    assert!(result.is_ok());
    assert!(finished.load(Ordering::Acquire));
    let output = read_result.expect("read processed PCM");
    assert_samples_close(&decode(&output), &samples);
}

#[tokio::test]
async fn cancellation_reaches_decoder_while_bounded_output_is_full() {
    let finished = Arc::new(AtomicBool::new(false));
    let shutdown = Arc::new(AtomicBool::new(false));
    let decoder = FakeDecoder {
        bytes: encode(&[0.1, -0.1]),
        cursor: 0,
        chunk_bytes: 8,
        repeat: true,
        finished,
        shutdown: Arc::clone(&shutdown),
    };
    let config = test_config();
    let lifecycle = lifecycle();
    let worker = spawn_pcm_worker(
        Box::new(decoder),
        DecoderProcessSlot::untracked(),
        None,
        test_state(),
        config,
        Duration::ZERO,
        Arc::clone(&lifecycle),
    )
    .await
    .expect("spawn worker");
    let input = worker.input;
    tokio::time::sleep(Duration::from_millis(20)).await;

    timeout(
        Duration::from_secs(1),
        worker
            .controller
            .apply(DspMutation::Gain(LinearGain::SILENCE)),
    )
    .await
    .expect("control cannot be starved by backpressure")
    .expect("gain update");
    lifecycle.cancellation().cancel();
    timeout(Duration::from_secs(1), worker.task)
        .await
        .expect("worker cancellation")
        .expect("worker task");
    drop(input);
    assert!(shutdown.load(Ordering::Acquire));
}

fn lifecycle() -> Arc<TrackLifecycle> {
    let (events, _) = broadcast::channel(8);
    TrackLifecycle::new(
        GuildId::new(1).expect("guild"),
        uuid::Uuid::new_v4(),
        PlaybackGeneration::new(1),
        Arc::new(AtomicU64::new(1)),
        Cancellation::default(),
        events,
    )
}

fn test_config() -> PipelineConfig {
    PipelineConfig {
        block_frames: 2,
        pcm_buffer_bytes: 16,
        songbird_buffer_bytes: 8,
        control_capacity: 4,
        event_capacity: 8,
        transition_frames: 1,
        orbit_period: Duration::from_mins(1),
        shutdown_timeout: Duration::from_secs(1),
    }
}

fn test_state() -> DspState {
    DspState {
        preset: prepared_identity(),
        gain: LinearGain::UNITY,
        spatial_enabled: true,
        orbit_origin: HorizontalStereoPair::FRONT,
    }
}

fn prepared_identity() -> Arc<PreparedHrir> {
    let mut wav = std::io::Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(
            &mut wav,
            hound::WavSpec {
                channels: 14,
                sample_rate: 48_000,
                bits_per_sample: 32,
                sample_format: hound::SampleFormat::Float,
            },
        )
        .expect("WAVE writer");
        let mut channels = [0.0_f32; 14];
        channels[0] = 1.0;
        channels[7] = 1.0;
        for sample in channels {
            writer.write_sample(sample).expect("sample");
        }
        writer.finalize().expect("finalize");
    }
    wav.set_position(0);
    let hrir = load_hesuvi_wav(wav).expect("load HRIR");
    Arc::new(PreparedHrir::from_hesuvi(&hrir).expect("prepare HRIR"))
}

fn encode(samples: &[f32]) -> Vec<u8> {
    samples
        .iter()
        .flat_map(|sample| sample.to_le_bytes())
        .collect()
}

fn decode(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

fn assert_samples_close(actual: &[f32], expected: &[f32]) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        assert!((actual - expected).abs() < 0.000_01);
    }
}
