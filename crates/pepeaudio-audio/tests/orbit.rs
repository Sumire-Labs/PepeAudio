mod support;

use pepeaudio_audio::{
    HorizontalOrbitRenderer, HorizontalStereoPair, PreparedHrir, StereoRenderer, blend_for_azimuth,
};
use pepeaudio_hrir::VirtualDirection;
use support::{load_preset, zero_pairs};

#[test]
fn exact_anchors_and_rear_wrap_use_expected_directions() {
    let anchors = [
        (0.0, VirtualDirection::FrontCenter),
        (30.0, VirtualDirection::FrontLeft),
        (-30.0, VirtualDirection::FrontRight),
        (90.0, VirtualDirection::SideLeft),
        (-90.0, VirtualDirection::SideRight),
        (150.0, VirtualDirection::BackLeft),
        (-150.0, VirtualDirection::BackRight),
    ];
    for (azimuth, direction) in anchors {
        let blend = blend_for_azimuth(azimuth).expect("finite angle");
        assert_eq!(blend.first, direction);
        assert!((blend.first_gain - 1.0).abs() < 1.0e-6);
        assert!(blend.second_gain.abs() < 1.0e-6);
    }

    let rear = blend_for_azimuth(180.0).expect("rear angle");
    assert_eq!(rear.first, VirtualDirection::BackLeft);
    assert_eq!(rear.second, VirtualDirection::BackRight);
    assert!((rear.first_gain - std::f32::consts::FRAC_1_SQRT_2).abs() < 1.0e-6);
    assert!((rear.second_gain - std::f32::consts::FRAC_1_SQRT_2).abs() < 1.0e-6);
    let wrapped = blend_for_azimuth(-180.0).expect("wrapped rear");
    assert_eq!(rear.first, wrapped.first);
    assert_eq!(rear.second, wrapped.second);
    assert!((rear.first_gain - wrapped.first_gain).abs() < 1.0e-6);
    assert!((rear.second_gain - wrapped.second_gain).abs() < 1.0e-6);

    let side_midpoint = blend_for_azimuth(60.0).expect("front-to-side midpoint");
    assert_eq!(side_midpoint.first, VirtualDirection::FrontLeft);
    assert_eq!(side_midpoint.second, VirtualDirection::SideLeft);
    assert!((side_midpoint.first_gain - std::f32::consts::FRAC_1_SQRT_2).abs() < 1.0e-6);
    assert!((side_midpoint.second_gain - std::f32::consts::FRAC_1_SQRT_2).abs() < 1.0e-6);
}

#[test]
fn orbit_output_is_continuous_across_rear_wrap() {
    let mut pairs = zero_pairs(1);
    // Only left-ear response is needed. Make BL and BR intentionally different.
    pairs[3] = (vec![0.25], vec![0.0]);
    pairs[4] = (vec![0.75], vec![0.0]);
    let preset = PreparedHrir::from_hesuvi(&load_preset(48_000, &pairs)).expect("prepare");
    let initial = HorizontalStereoPair::new(179.0, 0.0).expect("position");
    let mut renderer = HorizontalOrbitRenderer::new(&preset, initial).expect("renderer");

    let mut before = [0.0; 2];
    renderer
        .render_block(&[1.0, 0.0], &mut before)
        .expect("before wrap");
    renderer.set_position(HorizontalStereoPair::new(-179.0, 0.0).expect("position"));
    let mut after = [0.0; 2];
    renderer
        .render_block(&[1.0, 0.0], &mut after)
        .expect("after wrap");

    assert!(
        (after[0] - before[0]).abs() < 0.03,
        "{before:?} -> {after:?}"
    );
    assert!(before[1].abs() < f32::EPSILON);
    assert!(after[1].abs() < f32::EPSILON);
}

#[test]
fn front_stereo_pair_hits_fl_and_fr_without_interpolation() {
    let position = HorizontalStereoPair::FRONT;
    assert!((position.left_degrees() - 30.0).abs() < f32::EPSILON);
    assert!((position.right_degrees() + 30.0).abs() < f32::EPSILON);
    assert_eq!(
        blend_for_azimuth(position.left_degrees())
            .expect("left")
            .first,
        VirtualDirection::FrontLeft
    );
    assert_eq!(
        blend_for_azimuth(position.right_degrees())
            .expect("right")
            .first,
        VirtualDirection::FrontRight
    );
}

#[test]
fn changing_to_an_inactive_direction_reuses_shared_input_history() {
    let mut pairs = zero_pairs(2);
    // Side-left was not selected at the initial front-center position. Its
    // delayed response must still see the sample written before the move.
    pairs[5] = (vec![0.0, 1.0], vec![0.0, 0.0]);
    let preset = PreparedHrir::from_hesuvi(&load_preset(48_000, &pairs)).expect("prepare");
    let front_center = HorizontalStereoPair::new(0.0, 0.0).expect("front center");
    let mut renderer = HorizontalOrbitRenderer::new(&preset, front_center).expect("renderer");

    renderer
        .render_block(&[1.0, 0.0], &mut [0.0; 2])
        .expect("prime shared history");
    renderer.set_position(HorizontalStereoPair::new(90.0, 0.0).expect("side left"));
    let mut output = [0.0; 2];
    renderer
        .render_block(&[0.0, 0.0], &mut output)
        .expect("render delayed side response");

    assert!((output[0] - 1.0).abs() < 1.0e-6, "{output:?}");
    assert!(output[1].abs() < f32::EPSILON);
}

#[test]
fn reset_discards_shared_history_for_every_direction() {
    let mut pairs = zero_pairs(2);
    pairs[5] = (vec![0.0, 1.0], vec![0.0, 0.0]);
    let preset = PreparedHrir::from_hesuvi(&load_preset(48_000, &pairs)).expect("prepare");
    let front_center = HorizontalStereoPair::new(0.0, 0.0).expect("front center");
    let mut renderer = HorizontalOrbitRenderer::new(&preset, front_center).expect("renderer");

    renderer
        .render_block(&[1.0, 0.0], &mut [0.0; 2])
        .expect("prime shared history");
    renderer.reset();
    renderer.set_position(HorizontalStereoPair::new(90.0, 0.0).expect("side left"));
    let mut output = [1.0; 2];
    renderer
        .render_block(&[0.0, 0.0], &mut output)
        .expect("render after reset");

    assert!(output.iter().all(|sample| sample.abs() < f32::EPSILON));
}
