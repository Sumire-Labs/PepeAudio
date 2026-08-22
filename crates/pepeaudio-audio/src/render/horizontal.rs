use super::{
    StereoRenderer,
    partitioned::{PartitionedStereoEngine, Selection},
};
use crate::{
    DspError, HorizontalStereoPair, PreparedHrir, blend_for_azimuth,
    signal::{ensure_finite_intermediate, validate_stereo_blocks},
};

/// The two input sources own shared frequency-domain histories. Moving across
/// an anchor or the rear wrap therefore never starts a cold direction filter.
/// This remains a horizontal approximation with no elevation component.
#[derive(Debug, Clone)]
pub struct HorizontalOrbitRenderer {
    engine: PartitionedStereoEngine,
    position: HorizontalStereoPair,
}

impl HorizontalOrbitRenderer {
    /// # Errors
    ///
    /// Returns an error when state-size arithmetic or FFT preparation fails.
    pub fn new(preset: &PreparedHrir, position: HorizontalStereoPair) -> Result<Self, DspError> {
        Ok(Self {
            engine: PartitionedStereoEngine::new(preset)?,
            position,
        })
    }

    /// Changes routing gains without rebuilding or clearing histories.
    pub const fn set_position(&mut self, position: HorizontalStereoPair) {
        self.position = position;
    }

    #[must_use]
    pub const fn position(&self) -> HorizontalStereoPair {
        self.position
    }

    pub(crate) fn warm_block_validated(&mut self, input: &[f32]) -> Result<(), DspError> {
        self.engine.warm_validated(input)
    }
}

impl StereoRenderer for HorizontalOrbitRenderer {
    fn render_block(&mut self, input: &[f32], output: &mut [f32]) -> Result<(), DspError> {
        validate_stereo_blocks(input, output)?;
        let left = Selection::from(blend_for_azimuth(self.position.left_degrees())?);
        let right = Selection::from(blend_for_azimuth(self.position.right_degrees())?);
        self.engine.render_validated(input, output, left, right)?;
        // Keep the wet-path headroom intact until the processor applies the
        // user gain and the final Songbird safety ceiling.
        for (index, sample) in output.iter_mut().enumerate() {
            *sample = ensure_finite_intermediate(index, *sample)?;
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.engine.reset();
    }
}
