use crate::{
    DspError, GainRamp, HorizontalStereoPair, LinearGain, PreparedHrir,
    renderer_state::{PreparedRenderer, RenderMode, RendererState},
    signal::{ensure_finite_output, validate_stereo_blocks},
    transition::{EqualPowerFade, UnitRamp, equal_power_weights_finite},
};

/// Construction and [`Self::switch_preset`] allocate filter state. Processing
/// uses fixed scratch buffers and performs no heap allocation.
#[derive(Debug, Clone)]
pub struct AudioProcessor {
    mode: RenderMode,
    active: RendererState,
    pending: Option<RendererState>,
    preset_fade: Option<EqualPowerFade>,
    wet_mix: UnitRamp,
    output_gain: GainRamp,
    active_scratch: Box<[f32]>,
    pending_scratch: Box<[f32]>,
    max_block_frames: usize,
}

impl AudioProcessor {
    /// # Errors
    ///
    /// Returns an error for zero capacity or invalid prepared FIR data.
    pub fn new(
        preset: &PreparedHrir,
        mode: RenderMode,
        max_block_frames: usize,
    ) -> Result<Self, DspError> {
        Self::from_prepared(PreparedRenderer::new(preset, mode)?, max_block_frames)
    }

    /// # Errors
    ///
    /// Returns an error for zero or overflowing block capacity.
    pub fn from_prepared(
        prepared: PreparedRenderer,
        max_block_frames: usize,
    ) -> Result<Self, DspError> {
        if max_block_frames == 0 {
            return Err(DspError::ZeroBlockCapacity);
        }
        let scratch_samples = max_block_frames
            .checked_mul(2)
            .ok_or(DspError::ResampleLengthOverflow)?;
        Ok(Self {
            mode: prepared.mode,
            active: prepared.state,
            pending: None,
            preset_fade: None,
            wet_mix: UnitRamp::new(1.0),
            output_gain: GainRamp::new(LinearGain::UNITY),
            active_scratch: vec![0.0; scratch_samples].into_boxed_slice(),
            pending_scratch: vec![0.0; scratch_samples].into_boxed_slice(),
            max_block_frames,
        })
    }

    /// Prepares a replacement renderer and fades old/new wet output with
    /// equal-power weights. Filter allocation happens in this control call.
    ///
    /// # Errors
    ///
    /// Returns an error if another replacement is active or FIR construction
    /// fails. Zero or one frame replaces the preset immediately.
    pub fn switch_preset(
        &mut self,
        preset: &PreparedHrir,
        transition_frames: usize,
    ) -> Result<(), DspError> {
        let prepared = PreparedRenderer::new(preset, self.mode)?;
        self.install_prepared_renderer(prepared, transition_frames)
    }

    /// Installs convolution state built outside the realtime worker.
    ///
    /// The prepared state must use the same fixed/orbit topology. A stale
    /// prepared orbit position is updated to the processor's current position
    /// without rebuilding history or spectra.
    ///
    /// # Errors
    ///
    /// Returns an error when another transition is active or the renderer
    /// topology differs.
    pub fn install_prepared_renderer(
        &mut self,
        mut prepared: PreparedRenderer,
        transition_frames: usize,
    ) -> Result<(), DspError> {
        if self.pending.is_some() {
            return Err(DspError::PresetTransitionInProgress);
        }
        if !prepared.state.matches_mode(self.mode) {
            return Err(DspError::RendererModeMismatch);
        }
        if let RenderMode::HorizontalOrbit(position) = self.mode {
            prepared.state.set_orbit_position(position)?;
        }
        let replacement = prepared.state;
        if transition_frames <= 1 {
            self.active = replacement;
            self.active.reset();
        } else {
            self.pending = Some(replacement);
            self.preset_fade = Some(EqualPowerFade::new(transition_frames));
        }
        Ok(())
    }

    /// Both paths stay warm throughout an equal-power transition.
    pub fn set_wet_enabled(&mut self, enabled: bool, transition_frames: usize) {
        self.wet_mix
            .set_target(if enabled { 1.0 } else { 0.0 }, transition_frames);
    }

    pub fn set_output_gain(&mut self, gain: LinearGain, transition_frames: usize) {
        self.output_gain.set_target(gain, transition_frames);
    }

    /// Updates the stereo pair without resetting warm direction banks.
    ///
    /// # Errors
    ///
    /// Returns an error when this processor uses fixed-front mode.
    pub fn set_orbit_position(&mut self, position: HorizontalStereoPair) -> Result<(), DspError> {
        self.active.set_orbit_position(position)?;
        if let Some(pending) = &mut self.pending {
            pending.set_orbit_position(position)?;
        }
        self.mode = RenderMode::HorizontalOrbit(position);
        Ok(())
    }

    /// # Errors
    ///
    /// Returns an error for an invalid block, a block above configured
    /// capacity, invalid input, or non-finite DSP output.
    pub fn process_block(&mut self, input: &[f32], output: &mut [f32]) -> Result<(), DspError> {
        let frames = validate_stereo_blocks(input, output)?;
        if frames > self.max_block_frames {
            return Err(DspError::BlockTooLarge {
                actual: frames,
                maximum: self.max_block_frames,
            });
        }
        let samples = input.len();
        if self.is_fully_dry() {
            self.active.warm_block_validated(input)?;
            for (frame_index, (source, destination)) in input
                .chunks_exact(2)
                .zip(output.chunks_exact_mut(2))
                .enumerate()
            {
                let output_gain = self.output_gain.next_frame_gain();
                destination[0] = ensure_finite_output(frame_index * 2, source[0] * output_gain)?;
                destination[1] =
                    ensure_finite_output(frame_index * 2 + 1, source[1] * output_gain)?;
            }
            return Ok(());
        }
        self.active
            .render_block(input, &mut self.active_scratch[..samples])?;
        if let Some(pending) = &mut self.pending {
            pending.render_block(input, &mut self.pending_scratch[..samples])?;
        }

        for frame_index in 0..frames {
            let sample_index = frame_index * 2;
            let (old_gain, new_gain) = self
                .preset_fade
                .as_mut()
                .map_or((1.0, 0.0), EqualPowerFade::next_weights);
            let wet_mix = self.wet_mix.next();
            let (dry_gain, wet_gain) = equal_power_weights_finite(wet_mix);
            let output_gain = self.output_gain.next_frame_gain();

            for channel in 0..2 {
                let index = sample_index + channel;
                let wet =
                    self.active_scratch[index] * old_gain + self.pending_scratch[index] * new_gain;
                let mixed = input[index] * dry_gain + wet * wet_gain;
                output[index] = ensure_finite_output(index, mixed * output_gain)?;
            }
        }

        if self
            .preset_fade
            .as_ref()
            .is_some_and(EqualPowerFade::is_complete)
        {
            if let Some(replacement) = self.pending.take() {
                self.active = replacement;
            }
            self.preset_fade = None;
        }
        Ok(())
    }

    fn is_fully_dry(&self) -> bool {
        self.wet_mix.is_settled()
            && self.wet_mix.current() == 0.0
            && self.pending.is_none()
            && self.preset_fade.is_none()
    }

    /// Clears convolution history and settles transitions at their targets.
    ///
    /// If a preset replacement is active, the replacement becomes active.
    /// This is intended for seek, stop, and decoder-generation discontinuity.
    pub fn reset(&mut self) {
        if let Some(mut pending) = self.pending.take() {
            pending.reset();
            self.active = pending;
            self.preset_fade = None;
        } else {
            self.active.reset();
        }
        self.wet_mix.settle();
        self.output_gain.settle();
        self.active_scratch.fill(0.0);
        self.pending_scratch.fill(0.0);
    }

    #[must_use]
    pub const fn wet_mix(&self) -> f32 {
        self.wet_mix.current()
    }

    #[must_use]
    pub const fn output_gain(&self) -> f32 {
        self.output_gain.current()
    }

    #[must_use]
    pub const fn is_switching_preset(&self) -> bool {
        self.pending.is_some()
    }
}
