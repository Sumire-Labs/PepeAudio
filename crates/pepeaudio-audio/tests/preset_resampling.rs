mod support;

use pepeaudio_audio::{OUTPUT_SAMPLE_RATE_HZ, PreparedHrir};
use pepeaudio_hrir::{ALL_DIRECTIONS, HesuviSampleRate, VirtualDirection};
use support::{assert_close, load_preset, zero_pairs};

#[test]
fn forty_eight_kilohertz_planes_are_retained_exactly() {
    let mut pairs = zero_pairs(3);
    pairs[0] = (vec![0.0, 0.25, -0.5], vec![1.0, 0.0, 0.5]);
    let source = load_preset(48_000, &pairs);
    let prepared = PreparedHrir::from_hesuvi(&source).expect("prepare");

    assert_eq!(prepared.source_sample_rate(), HesuviSampleRate::Hz48000);
    assert_eq!(prepared.sample_rate_hz(), OUTPUT_SAMPLE_RATE_HZ);
    assert_eq!(prepared.frame_count(), 3);
    assert_close(
        prepared.pair(VirtualDirection::FrontLeft).left_ear(),
        &pairs[0].0,
        0.0,
    );
    assert_close(
        prepared.pair(VirtualDirection::FrontLeft).right_ear(),
        &pairs[0].1,
        0.0,
    );
}

#[test]
fn shared_44_1_to_48_grid_preserves_relative_plane_delay() {
    let mut pairs = zero_pairs(442);
    for (direction_index, (left, right)) in pairs.iter_mut().enumerate() {
        let left_onset = 147 + direction_index;
        let right_onset = 294 + direction_index;
        left[left_onset] = 1.0;
        right[right_onset] = 1.0;
    }
    let source = load_preset(44_100, &pairs);
    let prepared = PreparedHrir::from_hesuvi(&source).expect("resample");

    assert_eq!(prepared.source_sample_rate(), HesuviSampleRate::Hz44100);
    assert_eq!(prepared.frame_count(), 481);
    for (direction_index, direction) in ALL_DIRECTIONS.iter().copied().enumerate() {
        let pair = prepared.pair(direction);
        let left_peak = peak_index(pair.left_ear());
        let right_peak = peak_index(pair.right_ear());
        let source_delay = (294 + direction_index) - (147 + direction_index);
        let expected_delay = (source_delay * 160 + 73) / 147;
        assert_eq!(right_peak - left_peak, expected_delay);
    }
}

fn peak_index(samples: &[f32]) -> usize {
    samples
        .iter()
        .enumerate()
        .max_by(|(_, left), (_, right)| {
            left.abs()
                .partial_cmp(&right.abs())
                .expect("finite prepared samples")
        })
        .map(|(index, _)| index)
        .expect("non-empty plane")
}
