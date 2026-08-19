#![allow(dead_code)]

use std::io::Cursor;

use pepeaudio_hrir::{HesuviPreset, load_hesuvi_wav};

pub(crate) type PairPlanes = [(Vec<f32>, Vec<f32>); 7];

const MAP_14: [[usize; 2]; 7] = [[0, 1], [8, 7], [6, 13], [4, 5], [12, 11], [2, 3], [10, 9]];

pub(crate) fn load_preset(sample_rate: u32, pairs: &PairPlanes) -> HesuviPreset {
    let frame_count = pairs[0].0.len();
    assert!(frame_count > 0);
    for (left, right) in pairs {
        assert_eq!(left.len(), frame_count);
        assert_eq!(right.len(), frame_count);
    }

    let mut cursor = Cursor::new(Vec::new());
    {
        let specification = hound::WavSpec {
            channels: 14,
            sample_rate,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer = hound::WavWriter::new(&mut cursor, specification).expect("WAVE writer");
        for frame_index in 0..frame_count {
            let mut channels = [0.0_f32; 14];
            for (pair_index, [left_channel, right_channel]) in MAP_14.iter().enumerate() {
                channels[*left_channel] = pairs[pair_index].0[frame_index];
                channels[*right_channel] = pairs[pair_index].1[frame_index];
            }
            for sample in channels {
                writer.write_sample(sample).expect("write sample");
            }
        }
        writer.finalize().expect("finalize WAVE");
    }
    cursor.set_position(0);
    load_hesuvi_wav(cursor).expect("load generated HeSuVi preset")
}

pub(crate) fn zero_pairs(frames: usize) -> PairPlanes {
    std::array::from_fn(|_| (vec![0.0; frames], vec![0.0; frames]))
}

pub(crate) fn identity_front_pairs(scale: f32) -> PairPlanes {
    let mut pairs = zero_pairs(1);
    // HesuviPreset stable order: FL, FR, FC, BL, BR, SL, SR.
    pairs[0] = (vec![scale], vec![0.0]);
    pairs[1] = (vec![0.0], vec![scale]);
    pairs
}

#[allow(clippy::cast_possible_truncation)]
pub(crate) fn naive_convolution(input: &[f32], impulse: &[f32]) -> Vec<f32> {
    let mut output = vec![0.0; input.len()];
    for output_index in 0..input.len() {
        let mut accumulator = 0.0_f64;
        for tap_index in 0..=output_index.min(impulse.len() - 1) {
            accumulator +=
                f64::from(impulse[tap_index]) * f64::from(input[output_index - tap_index]);
        }
        output[output_index] = accumulator as f32;
    }
    output
}

pub(crate) fn assert_close(actual: &[f32], expected: &[f32], tolerance: f32) {
    assert_eq!(actual.len(), expected.len());
    for (index, (&actual, &expected)) in actual.iter().zip(expected).enumerate() {
        assert!(
            (actual - expected).abs() <= tolerance,
            "sample {index}: actual {actual}, expected {expected}, tolerance {tolerance}"
        );
    }
}
