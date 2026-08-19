mod fixed;
mod horizontal;
mod partitioned;

pub use fixed::FixedFrontRenderer;
pub use horizontal::HorizontalOrbitRenderer;

use crate::DspError;

/// Allocation-free block renderer for interleaved 48 kHz stereo.
pub trait StereoRenderer: Send {
    /// # Errors
    ///
    /// Returns an error for mismatched/partial blocks, invalid input samples,
    /// an internal transform failure, or a non-finite DSP result.
    fn render_block(&mut self, input: &[f32], output: &mut [f32]) -> Result<(), DspError>;

    fn reset(&mut self);
}
