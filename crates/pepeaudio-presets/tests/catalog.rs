use std::path::Path;

use pepeaudio_core::HrirPresetId;
use pepeaudio_presets::{CatalogError, CatalogLimits, HrirCatalog};

#[test]
fn loads_and_prepares_direct_hesuvi_wav_files_in_stable_order() {
    let directory = tempfile::tempdir().expect("temp directory");
    write_hesuvi(directory.path().join("Studio.wav").as_path(), 48_000, 14);
    write_hesuvi(directory.path().join("Cinema.wav").as_path(), 44_100, 7);
    std::fs::write(directory.path().join("LICENSE.txt"), "operator supplied")
        .expect("attribution file");

    let catalog = HrirCatalog::load(directory.path(), CatalogLimits::default()).expect("catalog");
    assert_eq!(catalog.len(), 2);
    assert_eq!(
        catalog
            .descriptors()
            .iter()
            .map(|item| item.display_name.as_str())
            .collect::<Vec<_>>(),
        vec!["Cinema", "Studio"]
    );
    assert!(
        catalog
            .get(&HrirPresetId::new("Studio").expect("ID"))
            .is_some()
    );
    assert_eq!(catalog.descriptors()[0].source_sample_rate_hz, 44_100);
    assert_eq!(catalog.descriptors()[0].prepared_frames, 9);
}

#[test]
fn rejects_wav_assets_before_reading_when_size_limit_is_exceeded() {
    let directory = tempfile::tempdir().expect("temp directory");
    write_hesuvi(directory.path().join("Large.wav").as_path(), 48_000, 14);
    let limits = CatalogLimits::new(4, 8, 64).expect("limits");
    let error = HrirCatalog::load(directory.path(), limits).expect_err("oversized asset");
    assert!(matches!(error, CatalogError::FileTooLarge { .. }));
}

#[test]
fn rejects_names_that_cannot_fit_a_discord_selector() {
    let directory = tempfile::tempdir().expect("temp directory");
    let name = format!("{}.wav", "x".repeat(101));
    write_hesuvi(directory.path().join(name).as_path(), 48_000, 7);
    let error =
        HrirCatalog::load(directory.path(), CatalogLimits::default()).expect_err("overlong option");
    assert!(matches!(error, CatalogError::InvalidIdentifier { .. }));
}

#[test]
fn enforces_the_realtime_limit_after_44k1_resampling() {
    let directory = tempfile::tempdir().expect("temp directory");
    write_hesuvi(directory.path().join("Expanded.wav").as_path(), 44_100, 7);
    let limits = CatalogLimits::new(4, 64 * 1024, 8)
        .and_then(|limits| limits.with_prepared_frame_limit(8))
        .expect("limits");

    let error = HrirCatalog::load(directory.path(), limits).expect_err("prepared limit");
    assert!(matches!(
        error,
        CatalogError::PreparedFramesTooLarge {
            actual: 9,
            maximum: 8,
            ..
        }
    ));
}

fn write_hesuvi(path: &Path, sample_rate: u32, channels: u16) {
    let spec = hound::WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec).expect("fixture writer");
    for frame in 0..8 {
        for channel in 0..channels {
            let sample = if frame == 0 && channel % 2 == 0 {
                1_000_i16
            } else {
                0_i16
            };
            writer.write_sample(sample).expect("fixture sample");
        }
    }
    writer.finalize().expect("fixture finalize");
}
