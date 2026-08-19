use std::f32::consts::FRAC_PI_2;

use crate::DspError;

/// Finite values outside `0..=1` are clamped.
///
/// # Errors
///
/// Returns an error for NaN or infinity.
pub fn equal_power_weights(progress: f32) -> Result<(f32, f32), DspError> {
    if !progress.is_finite() {
        return Err(DspError::InvalidTransitionProgress { actual: progress });
    }
    Ok(equal_power_weights_finite(progress))
}

pub(crate) fn equal_power_weights_finite(progress: f32) -> (f32, f32) {
    let progress = progress.clamp(0.0, 1.0);
    let angle = progress * FRAC_PI_2;
    (angle.cos(), angle.sin())
}

#[derive(Debug, Clone)]
pub(crate) struct UnitRamp {
    current: f32,
    target: f32,
    step: f32,
    remaining_frames: usize,
}

impl UnitRamp {
    pub(crate) const fn new(initial: f32) -> Self {
        Self {
            current: initial,
            target: initial,
            step: 0.0,
            remaining_frames: 0,
        }
    }

    #[allow(clippy::cast_precision_loss)]
    pub(crate) fn set_target(&mut self, target: f32, frames: usize) {
        self.target = target;
        if frames <= 1 {
            self.current = target;
            self.step = 0.0;
            self.remaining_frames = 0;
            return;
        }
        // See GainRamp: extremely long transitions may round their sub-sample
        // step without violating finite output or endpoint convergence.
        self.step = (target - self.current) / (frames - 1) as f32;
        self.remaining_frames = frames;
    }

    pub(crate) fn next(&mut self) -> f32 {
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

    pub(crate) const fn current(&self) -> f32 {
        self.current
    }

    pub(crate) const fn is_settled(&self) -> bool {
        self.remaining_frames == 0
    }

    pub(crate) fn settle(&mut self) {
        self.current = self.target;
        self.step = 0.0;
        self.remaining_frames = 0;
    }
}

#[derive(Debug, Clone)]
pub(crate) struct EqualPowerFade(UnitRamp);

impl EqualPowerFade {
    pub(crate) fn new(frames: usize) -> Self {
        let mut progress = UnitRamp::new(0.0);
        progress.set_target(1.0, frames);
        Self(progress)
    }

    pub(crate) fn next_weights(&mut self) -> (f32, f32) {
        equal_power_weights_finite(self.0.next())
    }

    pub(crate) const fn is_complete(&self) -> bool {
        self.0.is_settled()
    }
}
