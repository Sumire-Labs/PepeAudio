use std::{path::Path, process::Stdio, time::Duration};

use pepeaudio_media::{
    DecoderSpawner, Ffmpeg, Ffprobe, MediaProbe, OutputLimits, PcmDecoder, ProcessPool,
    RealProcessRunner,
};
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires ffmpeg and ffprobe executables on PATH"]
async fn probes_and_decodes_a_generated_audio_fixture() {
    let root = std::env::temp_dir().join(format!(
        "pepeaudio-ffmpeg-smoke-{}",
        Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(&root).expect("fixture root");
    let input = root.join("extensionless");
    let status = tokio::process::Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=0.1",
        ])
        .arg("-f")
        .arg("wav")
        .arg("-y")
        .arg(&input)
        .stdin(Stdio::null())
        .status()
        .await
        .expect("spawn fixture ffmpeg");
    assert!(status.success());

    let pool = ProcessPool::new(2).expect("pool");
    let probe = Ffprobe::new(
        "ffprobe",
        RealProcessRunner::new(pool.clone()),
        OutputLimits {
            timeout: Duration::from_secs(5),
            max_stdout_bytes: 128 * 1024,
            max_stderr_bytes: 16 * 1024,
        },
    );
    let metadata = probe.probe(Path::new(&input)).await.expect("probe");
    assert!(!metadata.audio_streams.is_empty());

    let ffmpeg =
        Ffmpeg::new("ffmpeg", pool, Duration::from_secs(5), 16 * 1024).expect("decoder config");
    let mut decoder = ffmpeg.spawn(&input).await.expect("spawn decoder");
    let mut pcm = [0_u8; 8_192];
    assert!(decoder.read_pcm(&mut pcm).await.expect("PCM") > 0);
    decoder.shutdown().await.expect("shutdown");

    let _ = std::fs::remove_dir_all(root);
}
