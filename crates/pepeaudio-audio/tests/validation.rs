mod support;

use pepeaudio_audio::{
    AudioProcessor, DirectFir, DspError, FirFilter, HorizontalStereoPair, PreparedHrir,
    PreparedRenderer, RenderMode,
};
use support::{identity_front_pairs, load_preset};

#[test]
fn malformed_blocks_are_rejected_before_rendering() {
    let prepared = PreparedHrir::from_hesuvi(&load_preset(48_000, &identity_front_pairs(1.0)))
        .expect("prepare");
    let mut processor =
        AudioProcessor::new(&prepared, RenderMode::FixedFront, 1).expect("processor");

    assert!(matches!(
        processor.process_block(&[0.0], &mut [0.0]),
        Err(DspError::OddStereoBlock { samples: 1 })
    ));
    assert!(matches!(
        processor.process_block(&[0.0, 0.0], &mut [0.0]),
        Err(DspError::BlockLengthMismatch {
            input: 2,
            output: 1
        })
    ));
    assert!(matches!(
        processor.process_block(&[0.0; 4], &mut [0.0; 4]),
        Err(DspError::BlockTooLarge {
            actual: 2,
            maximum: 1
        })
    ));
}

#[test]
fn prebuilt_replacements_install_only_into_matching_renderer_topology() {
    let prepared = PreparedHrir::from_hesuvi(&load_preset(48_000, &identity_front_pairs(0.5)))
        .expect("preset");
    let mut orbit = AudioProcessor::new(
        &prepared,
        RenderMode::HorizontalOrbit(HorizontalStereoPair::FRONT),
        1,
    )
    .expect("orbit processor");
    let fixed = PreparedRenderer::new(&prepared, RenderMode::FixedFront).expect("prebuild fixed");
    assert_eq!(
        orbit.install_prepared_renderer(fixed, 32),
        Err(DspError::RendererModeMismatch)
    );

    let stale_position = HorizontalStereoPair::new(90.0, 0.0).expect("stale position");
    let replacement = PreparedRenderer::new(&prepared, RenderMode::HorizontalOrbit(stale_position))
        .expect("prebuild orbit");
    orbit
        .install_prepared_renderer(replacement, 32)
        .expect("cheap install with current position");
    assert!(orbit.is_switching_preset());
}

#[test]
fn invalid_orbit_controls_and_overlapping_switches_are_rejected() {
    assert!(matches!(
        HorizontalStereoPair::new(f32::NAN, 60.0),
        Err(DspError::InvalidAzimuth { .. })
    ));
    assert!(matches!(
        HorizontalStereoPair::new(0.0, 181.0),
        Err(DspError::InvalidStereoWidth { .. })
    ));

    let first = PreparedHrir::from_hesuvi(&load_preset(48_000, &identity_front_pairs(1.0)))
        .expect("first preset");
    let second = PreparedHrir::from_hesuvi(&load_preset(48_000, &identity_front_pairs(0.5)))
        .expect("second preset");
    let mut processor = AudioProcessor::new(&first, RenderMode::FixedFront, 1).expect("processor");
    assert!(matches!(
        processor.set_orbit_position(HorizontalStereoPair::FRONT),
        Err(DspError::OrbitModeRequired)
    ));
    processor.switch_preset(&second, 10).expect("first switch");
    assert!(matches!(
        processor.switch_preset(&first, 10),
        Err(DspError::PresetTransitionInProgress)
    ));
}

#[test]
fn direct_fir_rejects_mismatched_blocks_without_advancing_history() {
    let mut filter = DirectFir::new(&[0.0, 1.0]).expect("delay FIR");
    assert!(matches!(
        filter.process_block(&[1.0], &mut []),
        Err(DspError::BlockLengthMismatch {
            input: 1,
            output: 0
        })
    ));
    let mut output = [1.0];
    filter
        .process_block(&[0.0], &mut output)
        .expect("valid block");
    assert!(output[0].abs() < f32::EPSILON);
}
