#![allow(clippy::cast_precision_loss)]

mod support;

use pepeaudio_audio::{
    DirectFir, FirFilter, FixedFrontRenderer, HorizontalOrbitRenderer, HorizontalStereoPair,
    PreparedHrir, StereoRenderer,
};
use support::{PairPlanes, assert_close, load_preset, zero_pairs};

const LONG_IR_FRAMES: usize = 1_025;

#[test]
fn partitioned_fixed_front_matches_direct_fir_across_irregular_blocks() {
    let pairs = long_impulses();
    let prepared = PreparedHrir::from_hesuvi(&load_preset(48_000, &pairs)).expect("prepare");
    let input = input_fixture(1_677);
    let expected = fixed_front_oracle(&input, &pairs);
    let mut actual = vec![0.0; input.len()];
    let mut renderer = FixedFrontRenderer::new(&prepared).expect("renderer");

    let block_frames = [1, 17, 238, 1, 256, 319, 7, 511, 327];
    let mut frame = 0;
    for frames in block_frames {
        let end = frame + frames;
        renderer
            .render_block(&input[frame * 2..end * 2], &mut actual[frame * 2..end * 2])
            .expect("render irregular block");
        frame = end;
    }
    assert_eq!(frame * 2, input.len());
    assert_close(&actual, &expected, 2.0e-5);
}

#[test]
fn long_inactive_direction_retains_history_across_partition_boundaries() {
    let mut pairs = zero_pairs(LONG_IR_FRAMES);
    pairs[5].0[777] = 0.75;
    let prepared = PreparedHrir::from_hesuvi(&load_preset(48_000, &pairs)).expect("prepare");
    let front = HorizontalStereoPair::new(0.0, 0.0).expect("front");
    let mut renderer = HorizontalOrbitRenderer::new(&prepared, front).expect("renderer");
    let mut prime = vec![0.0; 777 * 2];
    prime[0] = 1.0;
    renderer
        .render_block(&prime, &mut vec![0.0; prime.len()])
        .expect("prime every shared partition");

    renderer.set_position(HorizontalStereoPair::new(90.0, 0.0).expect("side"));
    let mut output = [0.0; 2];
    renderer
        .render_block(&[0.0, 0.0], &mut output)
        .expect("delayed inactive direction");
    assert_close(&output, &[0.75, 0.0], 2.0e-5);
}

#[test]
fn steady_state_render_and_direction_changes_do_not_allocate() {
    let prepared =
        PreparedHrir::from_hesuvi(&load_preset(48_000, &long_impulses())).expect("prepare");
    let mut renderer =
        HorizontalOrbitRenderer::new(&prepared, HorizontalStereoPair::FRONT).expect("renderer");
    let input = input_fixture(960);
    let mut output = vec![0.0; input.len()];
    renderer
        .render_block(&input, &mut output)
        .expect("warm implementation paths");

    let allocations = allocation_counter::measure(|| {
        renderer.set_position(HorizontalStereoPair::new(91.25, 60.0).expect("position"));
        renderer
            .render_block(&input, &mut output)
            .expect("allocation-free render");
        renderer.set_position(HorizontalStereoPair::new(-179.5, 60.0).expect("position"));
        renderer
            .render_block(&input, &mut output)
            .expect("allocation-free wrap render");
    });
    assert_eq!(allocations.count_total, 0, "{allocations:?}");
    assert_eq!(allocations.bytes_total, 0, "{allocations:?}");
}

fn fixed_front_oracle(input: &[f32], pairs: &PairPlanes) -> Vec<f32> {
    let left_input: Vec<_> = input.iter().step_by(2).copied().collect();
    let right_input: Vec<_> = input.iter().skip(1).step_by(2).copied().collect();
    let paths = [
        direct_path(&left_input, &pairs[0].0),
        direct_path(&left_input, &pairs[0].1),
        direct_path(&right_input, &pairs[1].0),
        direct_path(&right_input, &pairs[1].1),
    ];
    let mut output = Vec::with_capacity(input.len());
    for (((left_left, left_right), right_left), right_right) in
        paths[0].iter().zip(&paths[1]).zip(&paths[2]).zip(&paths[3])
    {
        output.push(left_left + right_left);
        output.push(left_right + right_right);
    }
    output
}

fn direct_path(input: &[f32], impulse: &[f32]) -> Vec<f32> {
    let mut filter = DirectFir::new(impulse).expect("oracle FIR");
    let mut output = vec![0.0; input.len()];
    filter
        .process_block(input, &mut output)
        .expect("oracle convolution");
    output
}

fn input_fixture(frames: usize) -> Vec<f32> {
    (0..frames)
        .flat_map(|frame| {
            let phase = frame as f32;
            [(phase * 0.037).sin() * 0.08, (phase * 0.061).cos() * 0.07]
        })
        .collect()
}

fn long_impulses() -> PairPlanes {
    std::array::from_fn(|direction| {
        let pair: [Vec<f32>; 2] = std::array::from_fn(|ear| {
            (0..LONG_IR_FRAMES)
                .map(|tap| {
                    let decay = (-8.0 * tap as f32 / LONG_IR_FRAMES as f32).exp();
                    let phase = (tap * (direction + 2) + ear * 13) as f32 * 0.19;
                    phase.sin() * decay * 0.000_8
                })
                .collect::<Vec<_>>()
        });
        (pair[0].clone(), pair[1].clone())
    })
}
