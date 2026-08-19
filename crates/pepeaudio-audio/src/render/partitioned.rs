use std::{fmt, ops::Range};

use pepeaudio_hrir::{ALL_DIRECTIONS, VirtualDirection};
use realfft::num_complex::Complex32;

use crate::{
    DirectionBlend, DspError, PreparedHrir,
    preset::direction_index,
    spectral::{FFT_FRAMES, FftPlans, PARTITION_FRAMES, SPECTRUM_BINS, SpectralPlane, fft_plans},
};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Selection {
    first: usize,
    second: usize,
    first_gain: f32,
    second_gain: f32,
}

impl Selection {
    pub(crate) fn exact(direction: VirtualDirection) -> Self {
        let index = direction_index(direction);
        Self {
            first: index,
            second: index,
            first_gain: 1.0,
            second_gain: 0.0,
        }
    }
}

impl From<DirectionBlend> for Selection {
    fn from(value: DirectionBlend) -> Self {
        Self {
            first: direction_index(value.first),
            second: direction_index(value.second),
            first_gain: value.first_gain,
            second_gain: value.second_gain,
        }
    }
}

#[derive(Clone, Debug)]
struct DirectionKernel {
    left: SpectralPlane,
    right: SpectralPlane,
}

#[derive(Clone)]
struct SourceHistory {
    plans: FftPlans,
    previous: Box<[f32]>,
    current: Box<[f32]>,
    time: Box<[f32]>,
    spectra: Box<[Complex32]>,
    scratch: Box<[Complex32]>,
    partition_count: usize,
    current_slot: usize,
    fill: usize,
}

impl fmt::Debug for SourceHistory {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SourceHistory")
            .field("partition_count", &self.partition_count)
            .field("current_slot", &self.current_slot)
            .field("fill", &self.fill)
            .finish_non_exhaustive()
    }
}

impl SourceHistory {
    fn new(partition_count: usize) -> Result<Self, DspError> {
        let plans = fft_plans();
        let spectrum_len = partition_count
            .checked_mul(SPECTRUM_BINS)
            .ok_or(DspError::ConvolutionBackend)?;
        let scratch_len = plans.forward.get_scratch_len();
        Ok(Self {
            plans,
            previous: vec![0.0; PARTITION_FRAMES].into_boxed_slice(),
            current: vec![0.0; PARTITION_FRAMES].into_boxed_slice(),
            time: vec![0.0; FFT_FRAMES].into_boxed_slice(),
            spectra: vec![Complex32::default(); spectrum_len].into_boxed_slice(),
            scratch: vec![Complex32::default(); scratch_len].into_boxed_slice(),
            partition_count,
            current_slot: 0,
            fill: 0,
        })
    }

    fn append_interleaved(
        &mut self,
        input: &[f32],
        channel: usize,
        frames: usize,
    ) -> Result<Range<usize>, DspError> {
        let start = self.fill;
        for (destination, source) in self.current[start..start + frames]
            .iter_mut()
            .zip(input[channel..].iter().step_by(2))
        {
            *destination = *source;
        }
        self.fill += frames;
        self.time[..PARTITION_FRAMES].copy_from_slice(&self.previous);
        self.time[PARTITION_FRAMES..].copy_from_slice(&self.current);
        let spectrum_start = self.current_slot * SPECTRUM_BINS;
        self.plans
            .forward
            .process_with_scratch(
                &mut self.time,
                &mut self.spectra[spectrum_start..spectrum_start + SPECTRUM_BINS],
                &mut self.scratch,
            )
            .map_err(|_| DspError::ConvolutionBackend)?;
        Ok(start..self.fill)
    }

    fn spectrum_at_delay(&self, delay: usize) -> &[Complex32] {
        let slot = (self.current_slot + self.partition_count - delay) % self.partition_count;
        let start = slot * SPECTRUM_BINS;
        &self.spectra[start..start + SPECTRUM_BINS]
    }

    fn finish_partition(&mut self) {
        if self.fill != PARTITION_FRAMES {
            return;
        }
        self.previous.copy_from_slice(&self.current);
        self.current.fill(0.0);
        self.fill = 0;
        self.current_slot = (self.current_slot + 1) % self.partition_count;
        let start = self.current_slot * SPECTRUM_BINS;
        self.spectra[start..start + SPECTRUM_BINS].fill(Complex32::default());
    }

    fn reset(&mut self) {
        self.previous.fill(0.0);
        self.current.fill(0.0);
        self.time.fill(0.0);
        self.spectra.fill(Complex32::default());
        self.scratch.fill(Complex32::default());
        self.current_slot = 0;
        self.fill = 0;
    }
}

#[derive(Clone)]
struct ConvolutionWorkspace {
    plans: FftPlans,
    spectrum: Box<[Complex32]>,
    time: Box<[f32]>,
    scratch: Box<[Complex32]>,
}

impl fmt::Debug for ConvolutionWorkspace {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConvolutionWorkspace")
            .finish_non_exhaustive()
    }
}

impl ConvolutionWorkspace {
    fn new() -> Self {
        let plans = fft_plans();
        let scratch_len = plans.inverse.get_scratch_len();
        Self {
            plans,
            spectrum: vec![Complex32::default(); SPECTRUM_BINS].into_boxed_slice(),
            time: vec![0.0; FFT_FRAMES].into_boxed_slice(),
            scratch: vec![Complex32::default(); scratch_len].into_boxed_slice(),
        }
    }

    fn convolve<'a>(
        &'a mut self,
        source: &SourceHistory,
        first: &SpectralPlane,
        second: &SpectralPlane,
        selection: Selection,
    ) -> Result<&'a [f32], DspError> {
        self.spectrum.fill(Complex32::default());
        for partition in 0..source.partition_count {
            let input = source.spectrum_at_delay(partition);
            let first_ir = first.partition(partition);
            let second_ir = second.partition(partition);
            for index in 0..SPECTRUM_BINS {
                let impulse = first_ir[index] * selection.first_gain
                    + second_ir[index] * selection.second_gain;
                self.spectrum[index] += input[index] * impulse;
            }
        }
        self.spectrum[0].im = 0.0;
        self.spectrum[SPECTRUM_BINS - 1].im = 0.0;
        self.plans
            .inverse
            .process_with_scratch(&mut self.spectrum, &mut self.time, &mut self.scratch)
            .map_err(|_| DspError::ConvolutionBackend)?;
        Ok(&self.time[PARTITION_FRAMES..])
    }
}

/// Two input spectra are shared by every directional ear plane. This avoids
/// both cold direction history and redundant forward transforms.
#[derive(Clone, Debug)]
pub(crate) struct PartitionedStereoEngine {
    kernels: [DirectionKernel; 7],
    left_source: SourceHistory,
    right_source: SourceHistory,
    workspace: ConvolutionWorkspace,
}

impl PartitionedStereoEngine {
    pub(crate) fn new(preset: &PreparedHrir) -> Result<Self, DspError> {
        let kernels = ALL_DIRECTIONS.map(|direction| {
            let pair = preset.pair(direction);
            DirectionKernel {
                left: pair.left_spectrum().clone(),
                right: pair.right_spectrum().clone(),
            }
        });
        let partition_count = kernels[0].left.partition_count();
        Ok(Self {
            kernels,
            left_source: SourceHistory::new(partition_count)?,
            right_source: SourceHistory::new(partition_count)?,
            workspace: ConvolutionWorkspace::new(),
        })
    }

    pub(crate) fn render_validated(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        left: Selection,
        right: Selection,
    ) -> Result<(), DspError> {
        let mut frame_offset = 0;
        while frame_offset < input.len() / 2 {
            let available = PARTITION_FRAMES - self.left_source.fill;
            let frames = available.min(input.len() / 2 - frame_offset);
            let samples = &input[frame_offset * 2..(frame_offset + frames) * 2];
            let range = self.left_source.append_interleaved(samples, 0, frames)?;
            self.right_source.append_interleaved(samples, 1, frames)?;
            self.render_range(output, frame_offset, range, left, right)?;
            self.left_source.finish_partition();
            self.right_source.finish_partition();
            frame_offset += frames;
        }
        Ok(())
    }

    pub(crate) fn warm_validated(&mut self, input: &[f32]) -> Result<(), DspError> {
        let mut frame_offset = 0;
        while frame_offset < input.len() / 2 {
            let frames =
                (PARTITION_FRAMES - self.left_source.fill).min(input.len() / 2 - frame_offset);
            let samples = &input[frame_offset * 2..(frame_offset + frames) * 2];
            self.left_source.append_interleaved(samples, 0, frames)?;
            self.right_source.append_interleaved(samples, 1, frames)?;
            self.left_source.finish_partition();
            self.right_source.finish_partition();
            frame_offset += frames;
        }
        Ok(())
    }

    fn render_range(
        &mut self,
        output: &mut [f32],
        output_frame: usize,
        range: Range<usize>,
        left: Selection,
        right: Selection,
    ) -> Result<(), DspError> {
        let output_start = output_frame * 2;
        let frames = range.len();
        let output = &mut output[output_start..output_start + frames * 2];
        output.fill(0.0);
        self.add_ear(output, 0, true, left, range.clone())?;
        self.add_ear(output, 1, true, left, range.clone())?;
        self.add_ear(output, 0, false, right, range.clone())?;
        self.add_ear(output, 1, false, right, range)?;
        Ok(())
    }

    fn add_ear(
        &mut self,
        output: &mut [f32],
        ear: usize,
        left_source: bool,
        selection: Selection,
        range: Range<usize>,
    ) -> Result<(), DspError> {
        let source = if left_source {
            &self.left_source
        } else {
            &self.right_source
        };
        let first = &self.kernels[selection.first];
        let second = &self.kernels[selection.second];
        let first = if ear == 0 { &first.left } else { &first.right };
        let second = if ear == 0 {
            &second.left
        } else {
            &second.right
        };
        let rendered = self.workspace.convolve(source, first, second, selection)?;
        for (destination, sample) in output[ear..].iter_mut().step_by(2).zip(&rendered[range]) {
            *destination += *sample;
        }
        Ok(())
    }

    pub(crate) fn reset(&mut self) {
        self.left_source.reset();
        self.right_source.reset();
        self.workspace.spectrum.fill(Complex32::default());
        self.workspace.time.fill(0.0);
        self.workspace.scratch.fill(Complex32::default());
    }
}
