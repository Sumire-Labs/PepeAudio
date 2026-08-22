use pepeaudio_hrir::VirtualDirection;

use super::{
    StereoRenderer,
    partitioned::{PartitionedStereoEngine, Selection},
};
use crate::{
    DspError, PreparedHrir,
    signal::{ensure_finite_intermediate, validate_stereo_blocks},
};

/// Faithful fixed-front stereo renderer using FL for the left input and FR for
/// the right input.
#[derive(Debug, Clone)]
pub struct FixedFrontRenderer {
    engine: PartitionedStereoEngine,
}

impl FixedFrontRenderer {
    /// # Errors
    ///
    /// Returns an error when state-size arithmetic or FFT preparation fails.
    pub fn new(preset: &PreparedHrir) -> Result<Self, DspError> {
        Ok(Self {
            engine: PartitionedStereoEngine::new(preset)?,
        })
    }

    pub(crate) fn warm_block_validated(&mut self, input: &[f32]) -> Result<(), DspError> {
        self.engine.warm_validated(input)
    }
}

impl StereoRenderer for FixedFrontRenderer {
    fn render_block(&mut self, input: &[f32], output: &mut [f32]) -> Result<(), DspError> {
        validate_stereo_blocks(input, output)?;
        self.engine.render_validated(
            input,
            output,
            Selection::exact(VirtualDirection::FrontLeft),
            Selection::exact(VirtualDirection::FrontRight),
        )?;
        // Preserve convolution headroom until AudioProcessor applies the user
        // gain. Clipping the wet signal here permanently distorted bass peaks
        // even when the final playback volume was well below unity.
        for (index, sample) in output.iter_mut().enumerate() {
            *sample = ensure_finite_intermediate(index, *sample)?;
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.engine.reset();
    }
}
