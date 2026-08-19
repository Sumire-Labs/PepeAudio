use crate::{DspError, signal::ensure_finite_output};

/// Highest accepted linear user/output gain (+12.04 dB).
pub const MAX_LINEAR_GAIN: f32 = 4.0;

/// Validated non-negative linear amplitude gain.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinearGain(f32);

impl LinearGain {
    pub const SILENCE: Self = Self(0.0);
    pub const UNITY: Self = Self(1.0);

    /// # Errors
    ///
    /// Returns an error for NaN, infinity, a negative value, or a value above
    /// [`MAX_LINEAR_GAIN`].
    pub fn new(value: f32) -> Result<Self, DspError> {
        if value.is_finite() && (0.0..=MAX_LINEAR_GAIN).contains(&value) {
            Ok(Self(value))
        } else {
            Err(DspError::InvalidGain {
                actual: value,
                maximum: MAX_LINEAR_GAIN,
            })
        }
    }

    #[must_use]
    pub const fn get(self) -> f32 {
        self.0
    }
}

/// For interleaved audio, one gain is used for every channel in a frame. A
/// ramp of at least two frames includes both its old and target endpoints.
#[derive(Debug, Clone)]
pub struct GainRamp {
    current: f32,
    target: f32,
    step: f32,
    remaining_frames: usize,
}

impl GainRamp {
    #[must_use]
    pub const fn new(initial: LinearGain) -> Self {
        Self {
            current: initial.get(),
            target: initial.get(),
            step: 0.0,
            remaining_frames: 0,
        }
    }

    /// Sets a target. Zero or one frame applies it immediately.
    #[allow(clippy::cast_precision_loss)]
    pub fn set_target(&mut self, target: LinearGain, frames: usize) {
        self.target = target.get();
        if frames <= 1 {
            self.current = self.target;
            self.step = 0.0;
            self.remaining_frames = 0;
            return;
        }
        // A transition longer than f32's exact integer range is not a useful
        // audio ramp; accepting it merely rounds the sub-sample step size.
        self.step = (self.target - self.current) / (frames - 1) as f32;
        self.remaining_frames = frames;
    }

    pub fn next_frame_gain(&mut self) -> f32 {
        let value = self.current;
        if self.remaining_frames > 0 {
            self.remaining_frames -= 1;
            if self.remaining_frames == 0 {
                self.current = self.target;
            } else {
                self.current += self.step;
            }
        }
        value
    }

    /// # Errors
    ///
    /// Returns an error when `channels` is zero, the block contains a partial
    /// frame, or multiplication produces a non-finite value.
    pub fn apply_interleaved(
        &mut self,
        samples: &mut [f32],
        channels: usize,
    ) -> Result<(), DspError> {
        if channels == 0 || !samples.len().is_multiple_of(channels) {
            return Err(DspError::InvalidInterleavedBlock {
                samples: samples.len(),
                channels,
            });
        }
        for (frame_index, frame) in samples.chunks_exact_mut(channels).enumerate() {
            let gain = self.next_frame_gain();
            for (channel_index, sample) in frame.iter_mut().enumerate() {
                *sample =
                    ensure_finite_output(frame_index * channels + channel_index, *sample * gain)?;
            }
        }
        Ok(())
    }

    #[must_use]
    pub const fn current(&self) -> f32 {
        self.current
    }

    pub(crate) fn settle(&mut self) {
        self.current = self.target;
        self.step = 0.0;
        self.remaining_frames = 0;
    }
}
