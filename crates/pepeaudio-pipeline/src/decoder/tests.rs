use std::{ffi::OsString, path::PathBuf, time::Duration};

use super::{DecoderFactory, FfmpegDecoderFactory, process::drain_bounded_discard};
use crate::{PipelineError, ResolvedSource};

fn factory() -> FfmpegDecoderFactory {
    FfmpegDecoderFactory::new(
        "ffmpeg",
        2,
        Duration::from_secs(1),
        Duration::from_secs(1),
        64 * 1024,
        Duration::from_hours(6),
    )
    .expect("valid factory")
}

#[test]
fn rejects_zero_and_empty_limits() {
    assert!(matches!(
        FfmpegDecoderFactory::new(
            "",
            1,
            Duration::from_secs(1),
            Duration::from_secs(1),
            1,
            Duration::from_secs(1),
        ),
        Err(PipelineError::InvalidConfig)
    ));
    assert!(matches!(
        FfmpegDecoderFactory::new(
            "ffmpeg",
            0,
            Duration::from_secs(1),
            Duration::from_secs(1),
            1,
            Duration::from_secs(1),
        ),
        Err(PipelineError::InvalidConfig)
    ));
    assert!(matches!(
        FfmpegDecoderFactory::new(
            "ffmpeg",
            1,
            Duration::ZERO,
            Duration::from_secs(1),
            1,
            Duration::from_secs(1),
        ),
        Err(PipelineError::InvalidConfig)
    ));
}

#[test]
fn command_is_an_argument_array_with_fixed_pcm_format() {
    let source = ResolvedSource::new(PathBuf::from("-untrusted media.bin"));
    let specification = factory().command_spec(&source, Duration::from_millis(1_500));
    let arguments = specification.arguments();

    assert_eq!(specification.program(), std::path::Path::new("ffmpeg"));
    assert!(contains_pair(arguments, "-protocol_whitelist", "file,pipe"));
    assert!(contains_pair(arguments, "-ss", "1.500000000"));
    assert!(contains_pair(arguments, "-t", "21600.000000000"));
    assert!(contains_pair(arguments, "-f", "f32le"));
    assert!(contains_pair(arguments, "-acodec", "pcm_f32le"));
    assert!(contains_pair(arguments, "-ar", "48000"));
    assert!(contains_pair(arguments, "-ac", "2"));
    assert!(contains_pair(arguments, "-i", "-untrusted media.bin"));
    assert_eq!(arguments.last(), Some(&OsString::from("pipe:1")));
}

#[test]
fn offset_preserves_nanosecond_precision_without_shell_text() {
    let source = ResolvedSource::new("managed-object");
    let specification = factory().command_spec(&source, Duration::new(9_007_199_254, 123_456_789));
    assert!(contains_pair(
        specification.arguments(),
        "-ss",
        "9007199254.123456789"
    ));
}

fn contains_pair(arguments: &[OsString], flag: &str, value: &str) -> bool {
    arguments
        .windows(2)
        .any(|pair| pair[0] == flag && pair[1] == value)
}

#[tokio::test]
async fn stderr_is_drained_with_constant_retention_and_reports_overflow() {
    use tokio::io::AsyncWriteExt as _;

    let (mut reader, mut writer) = tokio::io::duplex(32);
    let write = tokio::spawn(async move {
        writer
            .write_all(&[7_u8; 257])
            .await
            .expect("write diagnostics");
    });
    assert!(
        drain_bounded_discard(&mut reader, 256)
            .await
            .expect("drain diagnostics")
    );
    write.await.expect("writer task");
}

#[tokio::test]
async fn stderr_at_limit_is_not_reported_as_overflow() {
    use tokio::io::AsyncWriteExt as _;

    let (mut reader, mut writer) = tokio::io::duplex(32);
    let write = tokio::spawn(async move {
        writer
            .write_all(&[3_u8; 256])
            .await
            .expect("write diagnostics");
    });
    assert!(
        !drain_bounded_discard(&mut reader, 256)
            .await
            .expect("drain diagnostics")
    );
    write.await.expect("writer task");
}

#[tokio::test]
#[ignore = "requires an installed ffmpeg executable"]
async fn installed_ffmpeg_decodes_f32_pcm_and_reaps() {
    let path = write_wav_fixture();

    let source = ResolvedSource::new(&path);
    let spawned = factory()
        .spawn(&source, Duration::ZERO)
        .await
        .expect("spawn ffmpeg");
    let (mut decoder, process_slot, replacement_permit) = spawned.into_parts();
    assert!(replacement_permit.is_none());
    let mut total = 0_usize;
    let mut buffer = [0_u8; 1_024];
    loop {
        let read = decoder.read_pcm(&mut buffer).await.expect("read PCM");
        if read == 0 {
            break;
        }
        total += read;
    }
    decoder.finish().await.expect("reap ffmpeg");
    drop(process_slot);
    assert_eq!(total, 960 * 2 * size_of::<f32>());
    tokio::fs::remove_file(path)
        .await
        .expect("remove exact fixture");
}

#[tokio::test]
#[ignore = "requires an installed ffmpeg executable"]
async fn installed_ffmpeg_replaces_at_the_stable_process_limit() {
    let path = write_wav_fixture();
    let source = ResolvedSource::new(&path);
    let factory = FfmpegDecoderFactory::new(
        "ffmpeg",
        1,
        Duration::from_secs(1),
        Duration::from_secs(1),
        64 * 1024,
        Duration::from_hours(6),
    )
    .expect("single-process factory");

    let active = factory
        .spawn(&source, Duration::ZERO)
        .await
        .expect("spawn active decoder");
    let (mut active_decoder, active_slot, _) = active.into_parts();
    let replacement = factory
        .spawn_replacement(&source, Duration::from_millis(10), &active_slot)
        .await
        .expect("spawn replacement while the stable slot is occupied");
    let (mut replacement_decoder, replacement_slot, replacement_permit) = replacement.into_parts();

    active_decoder
        .shutdown()
        .await
        .expect("stop active decoder");
    replacement_permit.expect("replacement permit").release();
    replacement_decoder
        .shutdown()
        .await
        .expect("stop replacement decoder");
    drop((active_slot, replacement_slot));
    tokio::fs::remove_file(path)
        .await
        .expect("remove exact fixture");
}

fn write_wav_fixture() -> PathBuf {
    let path = std::env::temp_dir().join(format!("pepeaudio-{}.wav", uuid::Uuid::new_v4()));
    let mut writer = hound::WavWriter::create(
        &path,
        hound::WavSpec {
            channels: 2,
            sample_rate: 48_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .expect("create fixture");
    for _ in 0..960 {
        writer.write_sample(0_i16).expect("left sample");
        writer.write_sample(0_i16).expect("right sample");
    }
    writer.finalize().expect("finalize fixture");
    path
}

#[tokio::test]
async fn full_stable_pool_still_admits_one_bounded_replacement() {
    let factory = FfmpegDecoderFactory::new(
        "ffmpeg",
        1,
        Duration::from_secs(1),
        Duration::from_secs(1),
        64 * 1024,
        Duration::from_hours(6),
    )
    .expect("valid factory");
    let active_slot = factory
        .acquire_process_slot()
        .await
        .expect("stable process slot");

    assert!(
        tokio::time::timeout(Duration::from_millis(20), factory.acquire_process_slot())
            .await
            .is_err(),
        "fresh admission must remain capped"
    );

    let replacement_permit = factory
        .acquire_replacement_permit(&active_slot)
        .await
        .expect("replacement transition at stable capacity");
    assert!(
        tokio::time::timeout(
            Duration::from_millis(20),
            factory.acquire_replacement_permit(&active_slot)
        )
        .await
        .is_err(),
        "only one decoder replacement may overlap globally"
    );

    replacement_permit.release();
    factory
        .acquire_replacement_permit(&active_slot)
        .await
        .expect("replacement capacity is returned after transition")
        .release();
    drop(active_slot);
    factory
        .acquire_process_slot()
        .await
        .expect("stable capacity is returned with the active slot");
}
