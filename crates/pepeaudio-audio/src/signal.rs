use crate::DspError;

/// Guard band for normalized decoded PCM entering this crate.
///
/// Values above unity are allowed for upstream overshoot, while pathological
/// finite values are rejected before they can overflow a convolution sum.
pub const MAX_ABS_INPUT_SAMPLE: f32 = 4.0;

/// Final safety ceiling before PCM is handed to Songbird.
///
/// This hard clip is a safety guard, not a mastering-quality limiter. HRIR
/// gain and perceptual loudness still require measured acceptance testing.
pub const MAX_ABS_OUTPUT_SAMPLE: f32 = 1.0;

pub(crate) fn validate_input(samples: &[f32]) -> Result<(), DspError> {
    for (index, &sample) in samples.iter().enumerate() {
        if !sample.is_finite() {
            return Err(DspError::NonFiniteInput { index });
        }
        if sample.abs() > MAX_ABS_INPUT_SAMPLE {
            return Err(DspError::InputSampleTooLarge {
                index,
                actual: sample.abs(),
                maximum: MAX_ABS_INPUT_SAMPLE,
            });
        }
    }
    Ok(())
}

pub(crate) fn validate_stereo_blocks(input: &[f32], output: &[f32]) -> Result<usize, DspError> {
    if input.len() != output.len() {
        return Err(DspError::BlockLengthMismatch {
            input: input.len(),
            output: output.len(),
        });
    }
    if !input.len().is_multiple_of(2) {
        return Err(DspError::OddStereoBlock {
            samples: input.len(),
        });
    }
    validate_input(input)?;
    Ok(input.len() / 2)
}

pub(crate) fn ensure_finite_output(index: usize, sample: f32) -> Result<f32, DspError> {
    if sample.is_finite() {
        Ok(sample.clamp(-MAX_ABS_OUTPUT_SAMPLE, MAX_ABS_OUTPUT_SAMPLE))
    } else {
        Err(DspError::NonFiniteOutput { index })
    }
}
