use std::{io::Read, path::Path};

use crate::{
    error::{LoadError, WaveSampleKind},
    model::{HesuviPreset, HesuviSampleRate, HrirPair, SourceLayout},
};

/// Default maximum number of frames in each source HRIR plane.
///
/// This permits two seconds at 48 kHz while bounding the primary allocation.
pub const DEFAULT_MAX_FRAMES: usize = 96_000;

/// Resource limits applied before sample buffers are allocated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoadLimits {
    max_frames: usize,
}

impl LoadLimits {
    #[must_use]
    pub const fn new(max_frames: usize) -> Self {
        Self { max_frames }
    }

    #[must_use]
    pub const fn max_frames(self) -> usize {
        self.max_frames
    }
}

impl Default for LoadLimits {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_FRAMES)
    }
}

// HeSuVi's 14-channel track order, expressed as [left ear, right ear] for:
// FL, FR, FC, BL, BR, SL, SR. Right-side entries are intentionally swapped
// relative to adjacent WAVE tracks.
const MAP_14: [[usize; 2]; 7] = [[0, 1], [8, 7], [6, 13], [4, 5], [12, 11], [2, 3], [10, 9]];

// A 7-channel file stores FL(L/R), SL(L/R), BL(L/R), FC. Its right-side
// directions are mirror expansions, not independent measurements.
const MAP_7: [[usize; 2]; 7] = [[0, 1], [1, 0], [6, 6], [4, 5], [5, 4], [2, 3], [3, 2]];

/// Both classic WAVE and `WAVE_FORMAT_EXTENSIBLE` are accepted when `hound`
/// 3.5.1 can decode their PCM16 or f32 sample representation.
///
/// # Errors
///
/// Returns [`LoadError`] when WAVE parsing fails or the input violates any
/// supported-format, resource-limit, or sample-integrity invariant.
pub fn load_hesuvi_wav<R: Read>(source: R) -> Result<HesuviPreset, LoadError> {
    load_hesuvi_wav_with_limits(source, LoadLimits::default())
}

/// # Errors
///
/// Returns [`LoadError`] when WAVE parsing fails or the input violates any
/// supported-format, resource-limit, or sample-integrity invariant.
pub fn load_hesuvi_wav_with_limits<R: Read>(
    source: R,
    limits: LoadLimits,
) -> Result<HesuviPreset, LoadError> {
    let reader = hound::WavReader::new(source)?;
    decode(reader, limits)
}

/// The path is used only to open the file. It is not retained or converted to
/// a preset ID or display name.
///
/// # Errors
///
/// Returns [`LoadError`] when the file cannot be opened or parsed, or when its
/// contents violate any supported-format, resource-limit, or sample-integrity
/// invariant.
pub fn load_hesuvi_wav_file(path: impl AsRef<Path>) -> Result<HesuviPreset, LoadError> {
    load_hesuvi_wav_file_with_limits(path, LoadLimits::default())
}

/// # Errors
///
/// Returns [`LoadError`] when the file cannot be opened or parsed, or when its
/// contents violate any supported-format, resource-limit, or sample-integrity
/// invariant.
pub fn load_hesuvi_wav_file_with_limits(
    path: impl AsRef<Path>,
    limits: LoadLimits,
) -> Result<HesuviPreset, LoadError> {
    let reader = hound::WavReader::open(path)?;
    decode(reader, limits)
}

fn decode<R: Read>(
    mut reader: hound::WavReader<R>,
    limits: LoadLimits,
) -> Result<HesuviPreset, LoadError> {
    let spec = reader.spec();
    let (source_layout, mapping) = validate_channels(spec.channels)?;
    let sample_rate = validate_sample_rate(spec.sample_rate)?;
    validate_encoding(spec.sample_format, spec.bits_per_sample)?;

    let duration = reader.duration();
    let frame_count = usize::try_from(duration).map_err(|_| LoadError::FrameCountOutOfRange {
        actual: u64::from(duration),
    })?;

    if frame_count == 0 {
        return Err(LoadError::ZeroLength);
    }
    if frame_count > limits.max_frames() {
        return Err(LoadError::TooManyFrames {
            actual: frame_count,
            maximum: limits.max_frames(),
        });
    }

    let channel_count = usize::from(spec.channels);
    let mut planes: Vec<Vec<f32>> = (0..channel_count)
        .map(|_| Vec::with_capacity(frame_count))
        .collect();

    match spec.sample_format {
        hound::SampleFormat::Int => {
            for (sample_index, sample) in reader.samples::<i16>().enumerate() {
                // Dividing by 2^15 maps the entire signed PCM16 domain to
                // [-1.0, 1.0) without overflowing the negative endpoint.
                let normalized = f32::from(sample?) / 32_768.0;
                push_sample(&mut planes, channel_count, sample_index, normalized)?;
            }
        }
        hound::SampleFormat::Float => {
            for (sample_index, sample) in reader.samples::<f32>().enumerate() {
                push_sample(&mut planes, channel_count, sample_index, sample?)?;
            }
        }
    }

    validate_plane_lengths(&planes, frame_count)?;
    let pairs = mapping.map(|[left, right]| HrirPair::from_planes(&planes[left], &planes[right]));

    Ok(HesuviPreset::new(
        sample_rate,
        source_layout,
        frame_count,
        pairs,
    ))
}

fn validate_channels(channels: u16) -> Result<(SourceLayout, &'static [[usize; 2]; 7]), LoadError> {
    match channels {
        7 => Ok((SourceLayout::SevenChannelMirrored, &MAP_7)),
        14 => Ok((SourceLayout::FourteenChannelIndependent, &MAP_14)),
        actual => Err(LoadError::UnsupportedChannelCount { actual }),
    }
}

fn validate_sample_rate(sample_rate: u32) -> Result<HesuviSampleRate, LoadError> {
    match sample_rate {
        44_100 => Ok(HesuviSampleRate::Hz44100),
        48_000 => Ok(HesuviSampleRate::Hz48000),
        actual => Err(LoadError::UnsupportedSampleRate { actual }),
    }
}

fn validate_encoding(
    sample_format: hound::SampleFormat,
    bits_per_sample: u16,
) -> Result<(), LoadError> {
    let supported = matches!(
        (sample_format, bits_per_sample),
        (hound::SampleFormat::Int, 16) | (hound::SampleFormat::Float, 32)
    );
    if supported {
        return Ok(());
    }

    let kind = match sample_format {
        hound::SampleFormat::Int => WaveSampleKind::Integer,
        hound::SampleFormat::Float => WaveSampleKind::Float,
    };
    Err(LoadError::UnsupportedSampleEncoding {
        kind,
        bits_per_sample,
    })
}

fn push_sample(
    planes: &mut [Vec<f32>],
    channel_count: usize,
    sample_index: usize,
    sample: f32,
) -> Result<(), LoadError> {
    let channel = sample_index % channel_count;
    let frame = sample_index / channel_count;
    if !sample.is_finite() {
        return Err(LoadError::NonFiniteSample { frame, channel });
    }
    planes[channel].push(sample);
    Ok(())
}

fn validate_plane_lengths(planes: &[Vec<f32>], expected: usize) -> Result<(), LoadError> {
    for (channel, plane) in planes.iter().enumerate() {
        let actual = plane.len();
        if actual != expected {
            return Err(LoadError::UnequalFrameCount {
                channel,
                expected,
                actual,
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_plane_lengths;
    use crate::LoadError;

    #[test]
    fn unequal_source_planes_are_rejected() {
        let mut planes = vec![vec![0.0; 3]; 7];
        planes[4].pop();

        let error = validate_plane_lengths(&planes, 3).unwrap_err();
        assert!(matches!(
            error,
            LoadError::UnequalFrameCount {
                channel: 4,
                expected: 3,
                actual: 2
            }
        ));
    }
}
