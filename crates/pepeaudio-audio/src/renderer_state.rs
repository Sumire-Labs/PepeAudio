use crate::{
    DspError, FixedFrontRenderer, HorizontalOrbitRenderer, HorizontalStereoPair, PreparedHrir,
    StereoRenderer,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RenderMode {
    /// Left input at FL +30° and right input at FR -30°.
    FixedFront,
    /// Horizontal seven-anchor approximation with no elevation.
    HorizontalOrbit(HorizontalStereoPair),
}

#[derive(Debug, Clone)]
pub(crate) enum RendererState {
    Fixed(Box<FixedFrontRenderer>),
    Orbit(Box<HorizontalOrbitRenderer>),
}

impl RendererState {
    pub(crate) fn new(preset: &PreparedHrir, mode: RenderMode) -> Result<Self, DspError> {
        match mode {
            RenderMode::FixedFront => Ok(Self::Fixed(Box::new(FixedFrontRenderer::new(preset)?))),
            RenderMode::HorizontalOrbit(position) => Ok(Self::Orbit(Box::new(
                HorizontalOrbitRenderer::new(preset, position)?,
            ))),
        }
    }

    pub(crate) fn render_block(
        &mut self,
        input: &[f32],
        output: &mut [f32],
    ) -> Result<(), DspError> {
        match self {
            Self::Fixed(renderer) => renderer.render_block(input, output),
            Self::Orbit(renderer) => renderer.render_block(input, output),
        }
    }

    pub(crate) fn reset(&mut self) {
        match self {
            Self::Fixed(renderer) => renderer.reset(),
            Self::Orbit(renderer) => renderer.reset(),
        }
    }

    pub(crate) fn warm_block_validated(&mut self, input: &[f32]) -> Result<(), DspError> {
        match self {
            Self::Fixed(renderer) => renderer.warm_block_validated(input),
            Self::Orbit(renderer) => renderer.warm_block_validated(input),
        }
    }

    pub(crate) fn set_orbit_position(
        &mut self,
        position: HorizontalStereoPair,
    ) -> Result<(), DspError> {
        match self {
            Self::Fixed(_) => Err(DspError::OrbitModeRequired),
            Self::Orbit(renderer) => {
                renderer.set_position(position);
                Ok(())
            }
        }
    }

    pub(crate) const fn matches_mode(&self, mode: RenderMode) -> bool {
        matches!(
            (self, mode),
            (Self::Fixed(_), RenderMode::FixedFront)
                | (Self::Orbit(_), RenderMode::HorizontalOrbit(_))
        )
    }
}

/// Fully allocated convolution state prepared outside the PCM worker.
///
/// Building this value performs FFT planning/state allocation. Installing it
/// into an [`crate::AudioProcessor`] only moves prepared buffers and starts the
/// fade.
#[derive(Debug, Clone)]
pub struct PreparedRenderer {
    pub(crate) mode: RenderMode,
    pub(crate) state: RendererState,
}

impl PreparedRenderer {
    /// # Errors
    ///
    /// Returns an error if convolution state cannot be prepared safely.
    pub fn new(preset: &PreparedHrir, mode: RenderMode) -> Result<Self, DspError> {
        Ok(Self {
            mode,
            state: RendererState::new(preset, mode)?,
        })
    }

    #[must_use]
    pub const fn mode(&self) -> RenderMode {
        self.mode
    }
}
