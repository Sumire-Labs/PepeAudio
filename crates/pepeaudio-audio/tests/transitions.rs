mod support;

use pepeaudio_audio::{
    AudioProcessor, DspError, GainRamp, LinearGain, PreparedHrir, RenderMode, equal_power_weights,
};
use support::{assert_close, identity_front_pairs, load_preset, zero_pairs};

#[test]
fn equal_power_weights_have_unit_power_and_exact_endpoints() {
    let start = equal_power_weights(0.0).expect("finite progress");
    assert!((start.0 - 1.0).abs() < f32::EPSILON);
    assert!(start.1.abs() < f32::EPSILON);
    let end = equal_power_weights(1.0).expect("finite progress");
    assert!(end.0.abs() < 1.0e-6);
    assert!((end.1 - 1.0).abs() < 1.0e-6);
    for step in 0_u16..=100 {
        let (first, second) =
            equal_power_weights(f32::from(step) / 100.0).expect("finite progress");
        assert!((first.mul_add(first, second * second) - 1.0).abs() < 1.0e-5);
    }
    assert!(matches!(
        equal_power_weights(f32::NAN),
        Err(DspError::InvalidTransitionProgress { .. })
    ));
}

#[test]
fn gain_ramp_is_sample_accurate_across_block_boundaries() {
    let mut ramp = GainRamp::new(LinearGain::SILENCE);
    ramp.set_target(LinearGain::UNITY, 4);
    let mut first = [1.0, 1.0, 1.0, 1.0];
    ramp.apply_interleaved(&mut first, 2)
        .expect("first two frames");
    let mut second = [1.0, 1.0, 1.0, 1.0];
    ramp.apply_interleaved(&mut second, 2)
        .expect("last two frames");
    assert_close(&first, &[0.0, 0.0, 1.0 / 3.0, 1.0 / 3.0], 1.0e-6);
    assert_close(&second, &[2.0 / 3.0, 2.0 / 3.0, 1.0, 1.0], 1.0e-6);
}

#[test]
fn output_gain_prevents_wet_peak_clipping_before_the_final_safety_ceiling() {
    let prepared = PreparedHrir::from_hesuvi(&load_preset(48_000, &identity_front_pairs(2.0)))
        .expect("prepare");
    let mut processor =
        AudioProcessor::new(&prepared, RenderMode::FixedFront, 1).expect("processor");
    processor.set_output_gain(LinearGain::new(0.1).expect("gain"), 0);

    let mut quiet_output = [0.0; 2];
    processor
        .process_block(&[1.0, 1.0], &mut quiet_output)
        .expect("render below the ceiling");
    assert_close(&quiet_output, &[0.2, 0.2], 1.0e-6);

    processor.set_output_gain(LinearGain::UNITY, 0);
    let mut loud_output = [0.0; 2];
    processor
        .process_block(&[1.0, 1.0], &mut loud_output)
        .expect("render at the ceiling");
    assert_close(&loud_output, &[1.0, 1.0], 0.0);
}

#[test]
fn wet_to_bypass_transition_is_equal_power_and_block_continuous() {
    let prepared = PreparedHrir::from_hesuvi(&load_preset(48_000, &identity_front_pairs(0.5)))
        .expect("prepare");
    let mut processor =
        AudioProcessor::new(&prepared, RenderMode::FixedFront, 4).expect("processor");
    processor.set_wet_enabled(false, 4);

    let input = [1.0; 8];
    let mut output = [0.0; 8];
    processor
        .process_block(&input[..4], &mut output[..4])
        .expect("first half");
    processor
        .process_block(&input[4..], &mut output[4..])
        .expect("second half");

    let expected_frames = [
        0.5,
        equal_power_weights(2.0 / 3.0).expect("finite").0
            + equal_power_weights(2.0 / 3.0).expect("finite").1 * 0.5,
        equal_power_weights(1.0 / 3.0).expect("finite").0
            + equal_power_weights(1.0 / 3.0).expect("finite").1 * 0.5,
        1.0,
    ];
    // wet_mix goes 1 -> 0, so dry/wet equal-power arguments reverse.
    for (frame, expected) in output.chunks_exact(2).zip(expected_frames) {
        let safety_clipped = expected.min(1.0);
        assert!((frame[0] - safety_clipped).abs() < 1.0e-5);
        assert!((frame[1] - safety_clipped).abs() < 1.0e-5);
    }
}

#[test]
fn preset_hot_swap_has_exact_endpoints_and_no_step_discontinuity() {
    let old = PreparedHrir::from_hesuvi(&load_preset(48_000, &identity_front_pairs(1.0)))
        .expect("old preset");
    let new = PreparedHrir::from_hesuvi(&load_preset(48_000, &identity_front_pairs(0.5)))
        .expect("new preset");
    let mut processor = AudioProcessor::new(&old, RenderMode::FixedFront, 16).expect("processor");
    processor.switch_preset(&new, 9).expect("start switch");

    let input = [1.0; 18];
    let mut output = [0.0; 18];
    processor
        .process_block(&input[..6], &mut output[..6])
        .expect("first transition block");
    processor
        .process_block(&input[6..], &mut output[6..])
        .expect("second transition block");
    assert!((output[0] - 1.0).abs() < 1.0e-6);
    assert!((output[16] - 0.5).abs() < 1.0e-6);
    assert!(!processor.is_switching_preset());
    for samples in output.chunks_exact(2).collect::<Vec<_>>().windows(2) {
        assert!((samples[1][0] - samples[0][0]).abs() < 0.2);
        assert!((samples[1][1] - samples[0][1]).abs() < 0.2);
    }
}

#[test]
fn processor_reset_discards_fir_history_after_seek_or_stop() {
    let mut pairs = zero_pairs(2);
    pairs[0] = (vec![0.0, 1.0], vec![0.0, 0.0]);
    pairs[1] = (vec![0.0, 0.0], vec![0.0, 1.0]);
    let prepared = PreparedHrir::from_hesuvi(&load_preset(48_000, &pairs)).expect("prepare");
    let mut processor =
        AudioProcessor::new(&prepared, RenderMode::FixedFront, 1).expect("processor");

    processor
        .process_block(&[1.0, 1.0], &mut [0.0; 2])
        .expect("prime delayed history");
    processor.reset();
    let mut output = [1.0; 2];
    processor
        .process_block(&[0.0, 0.0], &mut output)
        .expect("post-reset silence");
    assert_close(&output, &[0.0, 0.0], 0.0);
}

#[test]
fn settled_dry_fast_path_keeps_history_warm_for_immediate_wet_restore() {
    let mut pairs = zero_pairs(2);
    pairs[0] = (vec![0.0, 1.0], vec![0.0, 0.0]);
    let prepared = PreparedHrir::from_hesuvi(&load_preset(48_000, &pairs)).expect("prepare");
    let mut processor =
        AudioProcessor::new(&prepared, RenderMode::FixedFront, 1).expect("processor");
    processor.set_wet_enabled(false, 0);

    let mut dry = [0.0; 2];
    processor
        .process_block(&[1.0, 0.0], &mut dry)
        .expect("dry fast path");
    assert_close(&dry, &[1.0, 0.0], 0.0);

    processor.set_wet_enabled(true, 0);
    let mut restored = [0.0; 2];
    processor
        .process_block(&[0.0, 0.0], &mut restored)
        .expect("immediate wet restore");
    assert_close(&restored, &[1.0, 0.0], 0.0);
}
