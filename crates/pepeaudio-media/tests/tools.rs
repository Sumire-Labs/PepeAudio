use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use async_trait::async_trait;
#[cfg(unix)]
use pepeaudio_media::{CommandSpec, RealProcessRunner};
use pepeaudio_media::{
    DecoderSpawner, Ffmpeg, Ffprobe, MediaProbe, OutputLimits, ProcessError, ProcessOutput,
    ProcessPool, ProcessRunner,
};

#[derive(Clone)]
struct FakeRunner {
    output: Result<ProcessOutput, FakeFailure>,
    seen: Arc<Mutex<Vec<pepeaudio_media::CommandSpec>>>,
}

#[derive(Clone, Copy, Debug)]
enum FakeFailure {
    Invalid,
}

#[async_trait]
impl ProcessRunner for FakeRunner {
    async fn run(
        &self,
        specification: &pepeaudio_media::CommandSpec,
        _limits: OutputLimits,
    ) -> Result<ProcessOutput, ProcessError> {
        self.seen
            .lock()
            .expect("seen lock")
            .push(specification.clone());
        self.output
            .clone()
            .map_err(|FakeFailure::Invalid| ProcessError::InvalidProbe)
    }
}

fn limits() -> OutputLimits {
    OutputLimits {
        timeout: Duration::from_secs(1),
        max_stdout_bytes: 16_384,
        max_stderr_bytes: 1_024,
    }
}

fn successful_runner(json: &str) -> FakeRunner {
    FakeRunner {
        output: Ok(ProcessOutput {
            status_code: Some(0),
            stdout: json.as_bytes().to_vec(),
            stderr: Vec::new(),
        }),
        seen: Arc::new(Mutex::new(Vec::new())),
    }
}

#[tokio::test]
async fn ffprobe_uses_argument_array_and_parses_audio_json() {
    let runner = successful_runner(
        r#"{
          "streams": [
            {"index": 0, "codec_type": "video", "codec_name": "h264"},
            {"index": 1, "codec_type": "audio", "codec_name": "opus",
             "sample_rate": "48000", "channels": 2, "channel_layout": "stereo"}
          ],
          "format": {"format_name": "matroska,webm", "duration": "12.5"}
        }"#,
    );
    let seen = Arc::clone(&runner.seen);
    let probe = Ffprobe::new("ffprobe", runner, limits());

    let metadata = probe
        .probe(Path::new("managed object without extension"))
        .await
        .expect("probe metadata");

    assert_eq!(metadata.duration_seconds, Some(12.5));
    assert_eq!(metadata.audio_streams.len(), 1);
    assert_eq!(metadata.audio_streams[0].sample_rate_hz, Some(48_000));
    let specification = &seen.lock().expect("seen lock")[0];
    assert_eq!(specification.program(), Path::new("ffprobe"));
    let arguments: Vec<_> = specification
        .arguments()
        .iter()
        .map(|value| value.to_string_lossy())
        .collect();
    assert!(
        arguments
            .windows(2)
            .any(|pair| pair == ["-protocol_whitelist", "file,pipe"])
    );
    assert!(
        arguments
            .windows(2)
            .any(|pair| pair == ["-i", "managed object without extension"])
    );
}

#[tokio::test]
async fn ffprobe_rejects_metadata_without_audio() {
    let probe = Ffprobe::new(
        "ffprobe",
        successful_runner(r#"{"streams":[{"index":0,"codec_type":"video"}]}"#),
        limits(),
    );

    let error = probe
        .probe(Path::new("object"))
        .await
        .expect_err("audio is required");

    assert!(matches!(error, ProcessError::NoAudioStream));
}

#[test]
fn ffmpeg_spec_is_fixed_to_48khz_stereo_f32le_and_local_protocols() {
    let ffmpeg = Ffmpeg::new(
        PathBuf::from("ffmpeg"),
        ProcessPool::new(2).expect("pool"),
        Duration::from_secs(2),
        4_096,
    )
    .expect("ffmpeg");

    let specification = ffmpeg.command_spec(Path::new("-untrusted-name"));
    let arguments: Vec<_> = specification
        .arguments()
        .iter()
        .map(|value| value.to_string_lossy())
        .collect();

    assert!(
        arguments
            .windows(2)
            .any(|pair| pair == ["-protocol_whitelist", "file,pipe"])
    );
    assert!(
        arguments
            .windows(2)
            .any(|pair| pair == ["-i", "-untrusted-name"])
    );
    assert!(arguments.windows(2).any(|pair| pair == ["-f", "f32le"]));
    assert!(arguments.windows(2).any(|pair| pair == ["-ar", "48000"]));
    assert!(arguments.windows(2).any(|pair| pair == ["-ac", "2"]));
    assert_eq!(arguments.last().expect("output"), "pipe:1");
}

#[tokio::test]
async fn fake_process_failure_is_propagated() {
    let probe = Ffprobe::new(
        "ffprobe",
        FakeRunner {
            output: Err(FakeFailure::Invalid),
            seen: Arc::new(Mutex::new(Vec::new())),
        },
        limits(),
    );

    assert!(matches!(
        probe.probe(Path::new("object")).await,
        Err(ProcessError::InvalidProbe)
    ));
}

#[test]
fn process_pool_and_decoder_reject_zero_limits() {
    assert!(matches!(
        ProcessPool::new(0),
        Err(ProcessError::InvalidConfig)
    ));
    let pool = ProcessPool::new(1).expect("pool");
    assert!(matches!(
        Ffmpeg::new("ffmpeg", pool, Duration::ZERO, 1),
        Err(ProcessError::InvalidConfig)
    ));
}

fn _assert_decoder_spawner<T: DecoderSpawner>(_value: &T) {}

#[cfg(unix)]
#[tokio::test]
async fn timeout_kills_pipe_holding_process_group_descendants() {
    let marker = std::env::temp_dir().join(format!(
        "pepeaudio-process-group-{}",
        uuid::Uuid::new_v4().simple()
    ));
    let specification = CommandSpec::new(
        "sh",
        vec![
            "-c".into(),
            "(sleep 1; printf survived > \"$1\") & exit 0".into(),
            "sh".into(),
            marker.as_os_str().to_owned(),
        ],
    );
    let runner = RealProcessRunner::new(ProcessPool::new(1).expect("pool"));
    let started = tokio::time::Instant::now();

    let result = runner
        .run(
            &specification,
            OutputLimits {
                timeout: Duration::from_millis(100),
                max_stdout_bytes: 1024,
                max_stderr_bytes: 1024,
            },
        )
        .await;

    assert!(matches!(result, Err(ProcessError::Timeout)));
    assert!(started.elapsed() < Duration::from_secs(3));
    tokio::time::sleep(Duration::from_millis(1_200)).await;
    assert!(
        !marker.exists(),
        "grandchild survived its process-group kill"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn external_cancellation_kills_the_process_group_and_restores_the_pool() {
    let marker = std::env::temp_dir().join(format!(
        "pepeaudio-process-cancel-marker-{}",
        uuid::Uuid::new_v4().simple()
    ));
    let ready = marker.with_extension("ready");
    let specification = CommandSpec::new(
        "sh",
        vec![
            "-c".into(),
            "printf ready > \"$1\"; (sleep 1; printf survived > \"$2\") & sleep 30".into(),
            "sh".into(),
            ready.as_os_str().to_owned(),
            marker.as_os_str().to_owned(),
        ],
    );
    let runner = RealProcessRunner::new(ProcessPool::new(1).expect("pool"));
    let task_runner = runner.clone();
    let task = tokio::spawn(async move {
        task_runner
            .run(
                &specification,
                OutputLimits {
                    timeout: Duration::from_secs(10),
                    max_stdout_bytes: 1024,
                    max_stderr_bytes: 1024,
                },
            )
            .await
    });
    tokio::time::timeout(Duration::from_secs(2), async {
        while !ready.exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("child started");

    task.abort();
    assert!(task.await.expect_err("cancelled task").is_cancelled());
    runner
        .run(
            &CommandSpec::new("true", Vec::new()),
            OutputLimits {
                timeout: Duration::from_secs(1),
                max_stdout_bytes: 16,
                max_stderr_bytes: 16,
            },
        )
        .await
        .expect("pool permit restored");
    tokio::time::sleep(Duration::from_millis(1_200)).await;
    assert!(!marker.exists(), "grandchild survived cancelled runner");
    let _ = std::fs::remove_file(ready);
}
