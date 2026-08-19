//! Manual shared-FDL partitioned-convolution throughput probe.
//!
//! Run this ignored test in release mode. It intentionally has no timing
//! assertion because host load and CPU generations differ:
//!
//! `cargo test --release -p pepeaudio-audio --test orbit_throughput -- --ignored --nocapture`

#![allow(clippy::cast_precision_loss)]

mod support;

use std::{hint::black_box, time::Instant};

use pepeaudio_audio::{
    HorizontalOrbitRenderer, HorizontalStereoPair, PreparedHrir, StereoRenderer,
};
use support::{PairPlanes, load_preset};

const SAMPLE_RATE_HZ: usize = 48_000;
const AUDIO_SECONDS: usize = 10;
const BLOCK_FRAMES: usize = 960;
const TAP_COUNTS: [usize; 4] = [256, 4_800, 9_600, 19_200];

#[test]
#[ignore = "manual release-mode throughput measurement; no stable CI time threshold"]
fn measures_horizontal_orbit_partitioned_fdl() {
    let input = input_fixture();
    assert_eq!(input.len(), SAMPLE_RATE_HZ * AUDIO_SECONDS * 2);

    for taps in TAP_COUNTS {
        let preset = PreparedHrir::from_hesuvi(&load_preset(48_000, &impulse_fixture(taps)))
            .expect("prepare benchmark HRIR");
        let mut renderer = HorizontalOrbitRenderer::new(&preset, HorizontalStereoPair::FRONT)
            .expect("construct orbit renderer");
        let mut output = vec![0.0_f32; BLOCK_FRAMES * 2];
        let started = Instant::now();
        let mut checksum = 0.0_f64;

        for (block_index, block) in input.chunks_exact(BLOCK_FRAMES * 2).enumerate() {
            let center = (block_index as f32 * 2.75).rem_euclid(360.0) - 180.0;
            renderer.set_position(
                HorizontalStereoPair::new(center, 60.0).expect("finite orbit position"),
            );
            renderer
                .render_block(block, &mut output)
                .expect("render benchmark block");
            checksum += output
                .iter()
                .map(|sample| f64::from(sample.abs()))
                .sum::<f64>();
            black_box(&output);
        }

        let elapsed = started.elapsed();
        let elapsed_seconds = elapsed.as_secs_f64();
        let realtime_multiple = AUDIO_SECONDS as f64 / elapsed_seconds;
        black_box(checksum);
        assert!(
            checksum.is_finite() && checksum > 0.0,
            "checksum={checksum}"
        );
        eprintln!(
            "orbit_throughput taps={taps} audio_seconds={AUDIO_SECONDS} \
             elapsed_seconds={elapsed_seconds:.6} realtime_x={realtime_multiple:.3} \
             checksum={checksum:.6}"
        );
    }
}

fn input_fixture() -> Vec<f32> {
    let frames = SAMPLE_RATE_HZ * AUDIO_SECONDS;
    let mut input = Vec::with_capacity(frames * 2);
    for frame in 0..frames {
        let time = frame as f32 / SAMPLE_RATE_HZ as f32;
        let left = (time * 440.0 * std::f32::consts::TAU).sin() * 0.12
            + (time * 997.0 * std::f32::consts::TAU).sin() * 0.03;
        let right = (time * 554.37 * std::f32::consts::TAU).sin() * 0.11
            + (time * 1_291.0 * std::f32::consts::TAU).sin() * 0.025;
        input.extend_from_slice(&[left, right]);
    }
    input
}

fn impulse_fixture(taps: usize) -> PairPlanes {
    std::array::from_fn(|direction| {
        let left = impulse_plane(taps, direction, 0);
        let right = impulse_plane(taps, direction, 1);
        (left, right)
    })
}

fn impulse_plane(taps: usize, direction: usize, ear: usize) -> Vec<f32> {
    (0..taps)
        .map(|tap| {
            let normalized = tap as f32 / taps as f32;
            let decay = (-7.0 * normalized).exp();
            let phase = (tap * (direction + 3) + ear * 11) as f32 * 0.173;
            let body = phase.sin() * decay * 0.0015;
            if tap == 0 {
                body + 0.035 + direction as f32 * 0.001 + ear as f32 * 0.0005
            } else {
                body
            }
        })
        .collect()
}
