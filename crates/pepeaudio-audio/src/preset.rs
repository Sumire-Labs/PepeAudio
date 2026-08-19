use std::sync::Arc;

use pepeaudio_hrir::{ALL_DIRECTIONS, HesuviPreset, HesuviSampleRate, VirtualDirection};

use crate::{
    DspError, Ear,
    resample::resample_44_1_to_48,
    spectral::{SpectralBuilder, SpectralPlane},
};

const MAX_ABS_IR_SAMPLE: f32 = 16.0;
const MAX_ABSOLUTE_IR_GAIN: f64 = 256.0;

#[derive(Debug, Clone, PartialEq)]
pub struct PreparedHrirPair {
    left_ear: Arc<[f32]>,
    right_ear: Arc<[f32]>,
    left_spectrum: SpectralPlane,
    right_spectrum: SpectralPlane,
}

impl PreparedHrirPair {
    #[must_use]
    pub fn left_ear(&self) -> &[f32] {
        &self.left_ear
    }

    #[must_use]
    pub fn right_ear(&self) -> &[f32] {
        &self.right_ear
    }

    #[must_use]
    pub fn frame_count(&self) -> usize {
        self.left_ear.len()
    }

    pub(crate) const fn left_spectrum(&self) -> &SpectralPlane {
        &self.left_spectrum
    }

    pub(crate) const fn right_spectrum(&self) -> &SpectralPlane {
        &self.right_spectrum
    }
}

/// All fourteen planes share one resampling grid. The preparation step never
/// normalizes or time-aligns an individual plane.
#[derive(Debug, Clone, PartialEq)]
pub struct PreparedHrir {
    source_sample_rate: HesuviSampleRate,
    frame_count: usize,
    pairs: [PreparedHrirPair; 7],
}

impl PreparedHrir {
    /// # Errors
    ///
    /// Returns an error if output length arithmetic overflows or if a prepared
    /// coefficient violates finite-value or absolute-gain safety bounds.
    pub fn from_hesuvi(preset: &HesuviPreset) -> Result<Self, DspError> {
        let mut spectral = SpectralBuilder::new();
        let pairs = [
            prepare_pair(preset, VirtualDirection::FrontLeft, &mut spectral)?,
            prepare_pair(preset, VirtualDirection::FrontRight, &mut spectral)?,
            prepare_pair(preset, VirtualDirection::FrontCenter, &mut spectral)?,
            prepare_pair(preset, VirtualDirection::BackLeft, &mut spectral)?,
            prepare_pair(preset, VirtualDirection::BackRight, &mut spectral)?,
            prepare_pair(preset, VirtualDirection::SideLeft, &mut spectral)?,
            prepare_pair(preset, VirtualDirection::SideRight, &mut spectral)?,
        ];
        let frame_count = pairs[0].frame_count();
        for (index, pair) in pairs.iter().enumerate() {
            let direction = ALL_DIRECTIONS[index];
            check_length(direction, Ear::Left, pair.left_ear(), frame_count)?;
            check_length(direction, Ear::Right, pair.right_ear(), frame_count)?;
        }

        Ok(Self {
            source_sample_rate: preset.sample_rate(),
            frame_count,
            pairs,
        })
    }

    #[must_use]
    pub const fn source_sample_rate(&self) -> HesuviSampleRate {
        self.source_sample_rate
    }

    #[must_use]
    pub const fn sample_rate_hz(&self) -> u32 {
        crate::OUTPUT_SAMPLE_RATE_HZ
    }

    #[must_use]
    pub const fn frame_count(&self) -> usize {
        self.frame_count
    }

    #[must_use]
    pub fn pair(&self, direction: VirtualDirection) -> &PreparedHrirPair {
        &self.pairs[direction_index(direction)]
    }
}

fn prepare_pair(
    preset: &HesuviPreset,
    direction: VirtualDirection,
    spectral: &mut SpectralBuilder,
) -> Result<PreparedHrirPair, DspError> {
    let pair = preset.pair(direction);
    let left = prepare_plane(pair.left_ear(), preset.sample_rate())?;
    let right = prepare_plane(pair.right_ear(), preset.sample_rate())?;
    validate_plane(&left)?;
    validate_plane(&right)?;
    Ok(PreparedHrirPair {
        left_spectrum: spectral.prepare(&left)?,
        right_spectrum: spectral.prepare(&right)?,
        left_ear: Arc::from(left),
        right_ear: Arc::from(right),
    })
}

fn prepare_plane(samples: &[f32], sample_rate: HesuviSampleRate) -> Result<Box<[f32]>, DspError> {
    match sample_rate {
        HesuviSampleRate::Hz44100 => resample_44_1_to_48(samples),
        HesuviSampleRate::Hz48000 => Ok(samples.into()),
    }
}

fn validate_plane(samples: &[f32]) -> Result<(), DspError> {
    if samples.is_empty() {
        return Err(DspError::EmptyImpulse);
    }
    let mut absolute_gain = 0.0_f64;
    for (index, &sample) in samples.iter().enumerate() {
        if !sample.is_finite() {
            return Err(DspError::NonFiniteImpulse { index });
        }
        if sample.abs() > MAX_ABS_IR_SAMPLE {
            return Err(DspError::ImpulseSampleTooLarge {
                index,
                actual: sample.abs(),
                maximum: MAX_ABS_IR_SAMPLE,
            });
        }
        absolute_gain += f64::from(sample.abs());
    }
    if absolute_gain > MAX_ABSOLUTE_IR_GAIN {
        return Err(DspError::ImpulseGainTooLarge {
            actual: absolute_gain,
            maximum: MAX_ABSOLUTE_IR_GAIN,
        });
    }
    Ok(())
}

fn check_length(
    direction: VirtualDirection,
    ear: Ear,
    samples: &[f32],
    expected: usize,
) -> Result<(), DspError> {
    if samples.len() == expected {
        Ok(())
    } else {
        Err(DspError::PreparedPlaneLength {
            direction,
            ear,
            actual: samples.len(),
            expected,
        })
    }
}

pub(crate) const fn direction_index(direction: VirtualDirection) -> usize {
    match direction {
        VirtualDirection::FrontLeft => 0,
        VirtualDirection::FrontRight => 1,
        VirtualDirection::FrontCenter => 2,
        VirtualDirection::BackLeft => 3,
        VirtualDirection::BackRight => 4,
        VirtualDirection::SideLeft => 5,
        VirtualDirection::SideRight => 6,
    }
}
