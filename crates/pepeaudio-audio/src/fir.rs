use std::sync::Arc;

use crate::{DspError, signal::validate_input};

const MAX_ABS_IR_SAMPLE: f32 = 16.0;
const MAX_ABSOLUTE_IR_GAIN: f64 = 256.0;

/// Safe block interface isolating a convolution backend from the renderer.
///
/// The production renderer has a separate shared-FDL partitioned engine; this
/// small interface keeps the direct numerical oracle independently testable.
pub trait FirFilter: Send {
    /// Overwrites `output` with the causal convolution for this block.
    ///
    /// History is preserved across calls, so arbitrary block boundaries are
    /// transparent.
    ///
    /// # Errors
    ///
    /// Returns an error for unequal block lengths or invalid input samples.
    fn process_block(&mut self, input: &[f32], output: &mut [f32]) -> Result<(), DspError>;

    fn reset(&mut self);

    fn impulse_len(&self) -> usize;
}

/// It is deterministic, has no process-time allocation, and serves as the
/// correctness oracle for the partitioned production implementation.
#[derive(Debug, Clone)]
pub struct DirectFir {
    impulse: Arc<[f32]>,
    history: Box<[f32]>,
    write_index: usize,
}

impl DirectFir {
    /// # Errors
    ///
    /// Returns an error when the impulse is empty, non-finite, or exceeds the
    /// coefficient and absolute-gain guards.
    pub fn new(impulse: &[f32]) -> Result<Self, DspError> {
        validate_impulse(impulse)?;
        Ok(Self::from_validated_shared(Arc::from(impulse)))
    }

    fn from_validated_shared(impulse: Arc<[f32]>) -> Self {
        Self {
            history: vec![0.0; impulse.len()].into_boxed_slice(),
            impulse,
            write_index: 0,
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    pub(crate) fn process_sample_validated(&mut self, input: f32) -> f32 {
        self.history[self.write_index] = input;
        let history_len = self.history.len();
        let mut accumulator = 0.0_f64;
        for (tap_index, &coefficient) in self.impulse.iter().enumerate() {
            let delay = tap_index % history_len;
            let history_index = (self.write_index + history_len - delay) % history_len;
            accumulator =
                f64::from(coefficient).mul_add(f64::from(self.history[history_index]), accumulator);
        }
        self.write_index = (self.write_index + 1) % history_len;
        // The public DSP sample format is f32; the f64 accumulator only reduces
        // rounding error. Construction guards bound the finite result tightly.
        accumulator as f32
    }
}

impl FirFilter for DirectFir {
    fn process_block(&mut self, input: &[f32], output: &mut [f32]) -> Result<(), DspError> {
        if input.len() != output.len() {
            return Err(DspError::BlockLengthMismatch {
                input: input.len(),
                output: output.len(),
            });
        }
        validate_input(input)?;
        for (&sample, destination) in input.iter().zip(output.iter_mut()) {
            *destination = self.process_sample_validated(sample);
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.history.fill(0.0);
        self.write_index = 0;
    }

    fn impulse_len(&self) -> usize {
        self.impulse.len()
    }
}

pub(crate) fn validate_impulse(impulse: &[f32]) -> Result<(), DspError> {
    if impulse.is_empty() {
        return Err(DspError::EmptyImpulse);
    }
    let mut absolute_gain = 0.0_f64;
    for (index, &coefficient) in impulse.iter().enumerate() {
        if !coefficient.is_finite() {
            return Err(DspError::NonFiniteImpulse { index });
        }
        if coefficient.abs() > MAX_ABS_IR_SAMPLE {
            return Err(DspError::ImpulseSampleTooLarge {
                index,
                actual: coefficient.abs(),
                maximum: MAX_ABS_IR_SAMPLE,
            });
        }
        absolute_gain += f64::from(coefficient.abs());
    }
    if absolute_gain > MAX_ABSOLUTE_IR_GAIN {
        return Err(DspError::ImpulseGainTooLarge {
            actual: absolute_gain,
            maximum: MAX_ABSOLUTE_IR_GAIN,
        });
    }
    Ok(())
}
