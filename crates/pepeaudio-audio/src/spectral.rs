use std::sync::{Arc, OnceLock};

use realfft::{ComplexToReal, RealFftPlanner, RealToComplex, num_complex::Complex32};

use crate::DspError;

/// Internal uniform-partition size. At 48 kHz this is 5.33 ms, while the
/// overlap-save implementation still emits the first sample without an
/// algorithmic block delay.
pub(crate) const PARTITION_FRAMES: usize = 256;
pub(crate) const FFT_FRAMES: usize = PARTITION_FRAMES * 2;
pub(crate) const SPECTRUM_BINS: usize = FFT_FRAMES / 2 + 1;

pub(crate) struct FftPlans {
    pub(crate) forward: Arc<dyn RealToComplex<f32>>,
    pub(crate) inverse: Arc<dyn ComplexToReal<f32>>,
}

impl Clone for FftPlans {
    fn clone(&self) -> Self {
        Self {
            forward: Arc::clone(&self.forward),
            inverse: Arc::clone(&self.inverse),
        }
    }
}

pub(crate) fn fft_plans() -> FftPlans {
    static PLANS: OnceLock<FftPlans> = OnceLock::new();
    PLANS
        .get_or_init(|| {
            let mut planner = RealFftPlanner::<f32>::new();
            FftPlans {
                forward: planner.plan_fft_forward(FFT_FRAMES),
                inverse: planner.plan_fft_inverse(FFT_FRAMES),
            }
        })
        .clone()
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SpectralPlane {
    partitions: Arc<[Complex32]>,
    partition_count: usize,
}

impl SpectralPlane {
    pub(crate) fn partition(&self, index: usize) -> &[Complex32] {
        let start = index * SPECTRUM_BINS;
        &self.partitions[start..start + SPECTRUM_BINS]
    }

    pub(crate) const fn partition_count(&self) -> usize {
        self.partition_count
    }
}

pub(crate) struct SpectralBuilder {
    plans: FftPlans,
    time: Box<[f32]>,
    spectrum: Box<[Complex32]>,
    scratch: Box<[Complex32]>,
}

impl SpectralBuilder {
    pub(crate) fn new() -> Self {
        let plans = fft_plans();
        let scratch =
            vec![Complex32::default(); plans.forward.get_scratch_len()].into_boxed_slice();
        Self {
            plans,
            time: vec![0.0; FFT_FRAMES].into_boxed_slice(),
            spectrum: vec![Complex32::default(); SPECTRUM_BINS].into_boxed_slice(),
            scratch,
        }
    }

    pub(crate) fn prepare(&mut self, impulse: &[f32]) -> Result<SpectralPlane, DspError> {
        let partition_count = impulse
            .len()
            .checked_add(PARTITION_FRAMES - 1)
            .ok_or(DspError::ConvolutionBackend)?
            / PARTITION_FRAMES;
        let spectral_len = partition_count
            .checked_mul(SPECTRUM_BINS)
            .ok_or(DspError::ConvolutionBackend)?;
        let mut partitions = Vec::with_capacity(spectral_len);
        // FFT_FRAMES is the fixed, small value 512 and is represented exactly.
        #[allow(clippy::cast_precision_loss)]
        let normalization = 1.0 / FFT_FRAMES as f32;

        for partition in impulse.chunks(PARTITION_FRAMES) {
            self.time.fill(0.0);
            self.time[..partition.len()].copy_from_slice(partition);
            self.plans
                .forward
                .process_with_scratch(&mut self.time, &mut self.spectrum, &mut self.scratch)
                .map_err(|_| DspError::ConvolutionBackend)?;
            partitions.extend(self.spectrum.iter().map(|value| *value * normalization));
        }
        Ok(SpectralPlane {
            partitions: partitions.into(),
            partition_count,
        })
    }
}
