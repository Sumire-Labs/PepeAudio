use std::io::Cursor;

use pepeaudio_hrir::{
    HesuviSampleRate, LoadError, LoadLimits, SourceLayout, VirtualDirection, load_hesuvi_wav,
    load_hesuvi_wav_with_limits,
};

const FLOAT_MARKERS: [f32; 14] = [
    0.01, 0.02, 0.03, 0.04, 0.05, 0.06, 0.07, 0.08, 0.09, 0.10, 0.11, 0.12, 0.13, 0.14,
];
const FRAME_OFFSETS: [f32; 4] = [0.0, 0.001, 0.002, 0.003];
const PCM_MARKERS: [i16; 7] = [-32_768, -24_576, -16_384, -8_192, 0, 8_192, 16_384];

#[test]
fn loads_classic_14_channel_float_with_exact_right_side_order() {
    let tracks = float_tracks(14, 3);
    let wave = classic_float_wave(48_000, &tracks);
    assert_eq!(format_tag(&wave), 3, "fixture must be classic IEEE float");

    let preset = load_hesuvi_wav(Cursor::new(wave)).unwrap();

    assert_eq!(preset.sample_rate(), HesuviSampleRate::Hz48000);
    assert_eq!(
        preset.source_layout(),
        SourceLayout::FourteenChannelIndependent
    );
    assert_eq!(preset.frame_count(), 3);

    assert_pair(&preset, VirtualDirection::FrontLeft, &tracks[0], &tracks[1]);
    assert_pair(
        &preset,
        VirtualDirection::FrontRight,
        &tracks[8],
        &tracks[7],
    );
    assert_pair(
        &preset,
        VirtualDirection::FrontCenter,
        &tracks[6],
        &tracks[13],
    );
    assert_pair(&preset, VirtualDirection::BackLeft, &tracks[4], &tracks[5]);
    assert_pair(
        &preset,
        VirtualDirection::BackRight,
        &tracks[12],
        &tracks[11],
    );
    assert_pair(&preset, VirtualDirection::SideLeft, &tracks[2], &tracks[3]);
    assert_pair(
        &preset,
        VirtualDirection::SideRight,
        &tracks[10],
        &tracks[9],
    );
}

#[test]
fn expands_extensible_7_channel_pcm16_by_mirroring_each_right_direction() {
    let tracks: Vec<Vec<i16>> = PCM_MARKERS
        .iter()
        .map(|marker| vec![*marker, marker.saturating_add(1_024)])
        .collect();
    let wave = extensible_pcm16_wave(44_100, &tracks);
    assert_eq!(
        format_tag(&wave),
        0xfffe,
        "hound must generate WAVE_FORMAT_EXTENSIBLE"
    );

    let preset = load_hesuvi_wav(Cursor::new(wave)).unwrap();
    let normalized: Vec<Vec<f32>> = tracks
        .iter()
        .map(|track| {
            track
                .iter()
                .map(|sample| f32::from(*sample) / 32_768.0)
                .collect()
        })
        .collect();

    assert_eq!(preset.sample_rate(), HesuviSampleRate::Hz44100);
    assert_eq!(preset.source_layout(), SourceLayout::SevenChannelMirrored);
    assert_eq!(preset.frame_count(), 2);

    assert_pair(
        &preset,
        VirtualDirection::FrontLeft,
        &normalized[0],
        &normalized[1],
    );
    assert_pair(
        &preset,
        VirtualDirection::FrontRight,
        &normalized[1],
        &normalized[0],
    );
    assert_pair(
        &preset,
        VirtualDirection::FrontCenter,
        &normalized[6],
        &normalized[6],
    );
    assert_pair(
        &preset,
        VirtualDirection::BackLeft,
        &normalized[4],
        &normalized[5],
    );
    assert_pair(
        &preset,
        VirtualDirection::BackRight,
        &normalized[5],
        &normalized[4],
    );
    assert_pair(
        &preset,
        VirtualDirection::SideLeft,
        &normalized[2],
        &normalized[3],
    );
    assert_pair(
        &preset,
        VirtualDirection::SideRight,
        &normalized[3],
        &normalized[2],
    );
}

#[test]
fn loads_extensible_14_channel_float() {
    let tracks = float_tracks(14, 1);
    let wave = extensible_float_wave(48_000, &tracks);
    assert_eq!(format_tag(&wave), 0xfffe);

    let preset = load_hesuvi_wav(Cursor::new(wave)).unwrap();

    assert_pair(
        &preset,
        VirtualDirection::FrontRight,
        &tracks[8],
        &tracks[7],
    );
    assert_pair(
        &preset,
        VirtualDirection::SideRight,
        &tracks[10],
        &tracks[9],
    );
}

#[test]
fn rejects_zero_length_input() {
    let tracks = vec![Vec::new(); 7];
    let error = load_hesuvi_wav(Cursor::new(classic_float_wave(48_000, &tracks))).unwrap_err();
    assert!(matches!(error, LoadError::ZeroLength));
}

#[test]
fn rejects_input_over_the_configured_frame_limit() {
    let tracks = float_tracks(7, 3);
    let error = load_hesuvi_wav_with_limits(
        Cursor::new(classic_float_wave(48_000, &tracks)),
        LoadLimits::new(2),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        LoadError::TooManyFrames {
            actual: 3,
            maximum: 2
        }
    ));
}

#[test]
fn rejects_non_finite_float_samples() {
    let mut tracks = float_tracks(7, 2);
    tracks[3][1] = f32::NAN;

    let error = load_hesuvi_wav(Cursor::new(classic_float_wave(48_000, &tracks))).unwrap_err();
    assert!(matches!(
        error,
        LoadError::NonFiniteSample {
            frame: 1,
            channel: 3
        }
    ));
}

#[test]
fn rejects_unsupported_channel_count_before_allocating_planes() {
    let tracks = float_tracks(8, 1);
    let error = load_hesuvi_wav(Cursor::new(classic_float_wave(48_000, &tracks))).unwrap_err();
    assert!(matches!(
        error,
        LoadError::UnsupportedChannelCount { actual: 8 }
    ));
}

#[test]
fn rejects_sample_rates_that_would_require_resampling() {
    let tracks = float_tracks(7, 1);
    let error = load_hesuvi_wav(Cursor::new(classic_float_wave(96_000, &tracks))).unwrap_err();
    assert!(matches!(
        error,
        LoadError::UnsupportedSampleRate { actual: 96_000 }
    ));
}

#[test]
fn rejects_non_pcm16_integer_depth() {
    let tracks = vec![vec![0_i32]; 7];
    let wave = extensible_pcm24_wave(48_000, &tracks);
    let error = load_hesuvi_wav(Cursor::new(wave)).unwrap_err();
    assert!(matches!(
        error,
        LoadError::UnsupportedSampleEncoding {
            kind: pepeaudio_hrir::WaveSampleKind::Integer,
            bits_per_sample: 24
        }
    ));
}

fn assert_pair(
    preset: &pepeaudio_hrir::HesuviPreset,
    direction: VirtualDirection,
    expected_left: &[f32],
    expected_right: &[f32],
) {
    let pair = preset.pair(direction);
    assert_eq!(pair.left_ear(), expected_left);
    assert_eq!(pair.right_ear(), expected_right);
    assert_eq!(pair.frame_count(), preset.frame_count());
}

fn float_tracks(channel_count: usize, frame_count: usize) -> Vec<Vec<f32>> {
    FLOAT_MARKERS[..channel_count]
        .iter()
        .map(|marker| {
            FRAME_OFFSETS[..frame_count]
                .iter()
                .map(|offset| marker + offset)
                .collect()
        })
        .collect()
}

fn classic_float_wave(sample_rate: u32, tracks: &[Vec<f32>]) -> Vec<u8> {
    let channel_count = u16::try_from(tracks.len()).unwrap();
    let frame_count = tracks.first().map_or(0, Vec::len);
    assert!(tracks.iter().all(|track| track.len() == frame_count));

    let bytes_per_sample = 4_u16;
    let block_align = channel_count * bytes_per_sample;
    let data_len = u32::try_from(frame_count)
        .unwrap()
        .checked_mul(u32::from(block_align))
        .unwrap();
    let mut bytes = Vec::with_capacity(usize::try_from(44 + data_len).unwrap());

    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVE");
    bytes.extend_from_slice(b"fmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&3_u16.to_le_bytes());
    bytes.extend_from_slice(&channel_count.to_le_bytes());
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(&(sample_rate * u32::from(block_align)).to_le_bytes());
    bytes.extend_from_slice(&block_align.to_le_bytes());
    bytes.extend_from_slice(&32_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());

    for frame in 0..frame_count {
        for track in tracks {
            bytes.extend_from_slice(&track[frame].to_le_bytes());
        }
    }
    bytes
}

fn extensible_pcm16_wave(sample_rate: u32, tracks: &[Vec<i16>]) -> Vec<u8> {
    write_with_hound(
        sample_rate,
        u16::try_from(tracks.len()).unwrap(),
        16,
        hound::SampleFormat::Int,
        |writer| {
            let frame_count = tracks.first().map_or(0, Vec::len);
            for frame in 0..frame_count {
                for track in tracks {
                    writer.write_sample(track[frame]).unwrap();
                }
            }
        },
    )
}

fn extensible_pcm24_wave(sample_rate: u32, tracks: &[Vec<i32>]) -> Vec<u8> {
    write_with_hound(
        sample_rate,
        u16::try_from(tracks.len()).unwrap(),
        24,
        hound::SampleFormat::Int,
        |writer| {
            let frame_count = tracks.first().map_or(0, Vec::len);
            for frame in 0..frame_count {
                for track in tracks {
                    writer.write_sample(track[frame]).unwrap();
                }
            }
        },
    )
}

fn extensible_float_wave(sample_rate: u32, tracks: &[Vec<f32>]) -> Vec<u8> {
    write_with_hound(
        sample_rate,
        u16::try_from(tracks.len()).unwrap(),
        32,
        hound::SampleFormat::Float,
        |writer| {
            let frame_count = tracks.first().map_or(0, Vec::len);
            for frame in 0..frame_count {
                for track in tracks {
                    writer.write_sample(track[frame]).unwrap();
                }
            }
        },
    )
}

fn write_with_hound<F>(
    sample_rate: u32,
    channels: u16,
    bits_per_sample: u16,
    sample_format: hound::SampleFormat,
    write_samples: F,
) -> Vec<u8>
where
    F: FnOnce(&mut hound::WavWriter<&mut Cursor<Vec<u8>>>),
{
    let mut cursor = Cursor::new(Vec::new());
    {
        let spec = hound::WavSpec {
            channels,
            sample_rate,
            bits_per_sample,
            sample_format,
        };
        let mut writer = hound::WavWriter::new(&mut cursor, spec).unwrap();
        write_samples(&mut writer);
        writer.finalize().unwrap();
    }
    cursor.into_inner()
}

fn format_tag(wave: &[u8]) -> u16 {
    u16::from_le_bytes([wave[20], wave[21]])
}
