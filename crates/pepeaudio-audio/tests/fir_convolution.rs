mod support;

use pepeaudio_audio::{
    DirectFir, DspError, FirFilter, LinearGain, MAX_ABS_INPUT_SAMPLE, MAX_LINEAR_GAIN,
};
use support::{assert_close, naive_convolution};

#[test]
fn delta_impulse_is_identity_and_zero_input_stays_zero() {
    let mut filter = DirectFir::new(&[1.0]).expect("valid delta");
    let input = [0.25, -0.5, 1.0, 0.0];
    let mut output = [0.0; 4];
    filter.process_block(&input, &mut output).expect("process");
    assert_close(&output, &input, 0.0);

    let mut zeros = [1.0; 4];
    filter
        .process_block(&[0.0; 4], &mut zeros)
        .expect("process zeros");
    assert_close(&zeros, &[0.0; 4], 0.0);
}

#[test]
fn direct_fir_matches_naive_golden_across_irregular_blocks() {
    let impulse = [0.5, -0.25, 0.125, 0.0625];
    let input = [0.2, -0.4, 0.1, 0.7, -0.3, 0.0, 0.8, -0.1, 0.25];
    let expected = naive_convolution(&input, &impulse);
    let mut actual = [0.0; 9];
    let mut filter = DirectFir::new(&impulse).expect("valid FIR");

    filter
        .process_block(&input[..2], &mut actual[..2])
        .expect("first block");
    filter
        .process_block(&input[2..7], &mut actual[2..7])
        .expect("middle block");
    filter
        .process_block(&input[7..], &mut actual[7..])
        .expect("last block");

    assert_close(&actual, &expected, 1.0e-6);
}

#[test]
fn reset_discards_delayed_history() {
    let mut filter = DirectFir::new(&[0.0, 1.0]).expect("valid delay");
    let mut first = [0.0];
    filter.process_block(&[1.0], &mut first).expect("impulse");
    assert_close(&first, &[0.0], 0.0);
    filter.reset();
    let mut after_reset = [1.0];
    filter
        .process_block(&[0.0], &mut after_reset)
        .expect("post-reset block");
    assert_close(&after_reset, &[0.0], 0.0);
}

#[test]
fn unsafe_finite_and_non_finite_values_are_rejected() {
    assert_eq!(
        DirectFir::new(&[]).expect_err("empty IR"),
        DspError::EmptyImpulse
    );
    assert!(matches!(
        DirectFir::new(&[f32::NAN]),
        Err(DspError::NonFiniteImpulse { index: 0 })
    ));
    let mut filter = DirectFir::new(&[1.0]).expect("valid FIR");
    assert!(matches!(
        filter.process_block(&[f32::INFINITY], &mut [0.0]),
        Err(DspError::NonFiniteInput { index: 0 })
    ));
    assert!(matches!(
        filter.process_block(&[MAX_ABS_INPUT_SAMPLE + 0.01], &mut [0.0]),
        Err(DspError::InputSampleTooLarge { index: 0, .. })
    ));
    assert!(matches!(
        DirectFir::new(&[1.0; 257]),
        Err(DspError::ImpulseGainTooLarge { .. })
    ));
    for invalid_gain in [-0.01, MAX_LINEAR_GAIN + 0.01, f32::NAN, f32::INFINITY] {
        assert!(matches!(
            LinearGain::new(invalid_gain),
            Err(DspError::InvalidGain { .. })
        ));
    }
}
