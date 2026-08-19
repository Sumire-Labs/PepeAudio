mod support;

use pepeaudio_audio::{FixedFrontRenderer, PreparedHrir, StereoRenderer};
use support::{assert_close, load_preset, zero_pairs};

#[test]
fn fixed_front_routes_both_input_channels_to_both_ears() {
    let mut pairs = zero_pairs(2);
    // FL and FR in HesuviPreset stable order.
    pairs[0] = (vec![1.0, 0.5], vec![0.25, 0.125]);
    pairs[1] = (vec![0.75, -0.25], vec![2.0, -0.5]);
    let preset = PreparedHrir::from_hesuvi(&load_preset(48_000, &pairs)).expect("prepare");
    let mut renderer = FixedFrontRenderer::new(&preset).expect("renderer");

    // A right-channel delta must use FR's left-ear and right-ear planes.
    let input = [0.0, 1.0, 0.0, 0.0];
    let mut output = [0.0; 4];
    renderer
        .render_block(&input, &mut output)
        .expect("render right source");
    assert_close(&output, &[0.75, 1.0, -0.25, -0.5], 1.0e-6);
}

#[test]
fn renderer_applies_a_final_finite_pcm_safety_ceiling() {
    let mut pairs = zero_pairs(1);
    pairs[0] = (vec![2.0], vec![-2.0]);
    let preset = PreparedHrir::from_hesuvi(&load_preset(48_000, &pairs)).expect("prepare");
    let mut renderer = FixedFrontRenderer::new(&preset).expect("renderer");
    let mut output = [0.0; 2];
    renderer
        .render_block(&[1.0, 0.0], &mut output)
        .expect("render");
    assert_close(&output, &[1.0, -1.0], 0.0);
}

#[test]
fn fixed_front_history_crosses_block_boundaries_without_changing_golden() {
    let mut pairs = zero_pairs(3);
    pairs[0] = (vec![0.5, 0.25, -0.125], vec![0.0; 3]);
    pairs[1] = (vec![0.0; 3], vec![1.0, -0.5, 0.25]);
    let preset = PreparedHrir::from_hesuvi(&load_preset(48_000, &pairs)).expect("prepare");
    let input = [1.0, 2.0, 0.0, 0.0, 0.0, 0.0];

    let mut whole_renderer = FixedFrontRenderer::new(&preset).expect("whole renderer");
    let mut whole = [0.0; 6];
    whole_renderer
        .render_block(&input, &mut whole)
        .expect("whole block");

    let mut split_renderer = FixedFrontRenderer::new(&preset).expect("split renderer");
    let mut split = [0.0; 6];
    split_renderer
        .render_block(&input[..2], &mut split[..2])
        .expect("first block");
    split_renderer
        .render_block(&input[2..], &mut split[2..])
        .expect("second block");

    assert_close(&split, &whole, 1.0e-6);
}
