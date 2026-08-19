//! Allocation-free steady-state audio DSP for `PepeAudio-rs`.
//!
//! The crate prepares validated [`pepeaudio_hrir::HesuviPreset`] values at
//! 48 kHz, precomputes uniform FFT partitions, renders fixed-front stereo, and
//! offers a seven-anchor horizontal orbit approximation. The `HeSuVi` backend
//! has no elevation data and is not a continuous spherical HRTF renderer.
//!
//! Parsing, resampling, filter construction, and preset replacement are
//! control-path operations. [`AudioProcessor::process_block`] performs no heap
//! allocation after construction.

#![forbid(unsafe_code)]

mod error;
mod fir;
mod gain;
mod orbit;
mod preset;
mod processor;
mod render;
mod renderer_state;
mod resample;
mod signal;
mod spectral;
mod transition;

pub use error::{DspError, Ear};
pub use fir::{DirectFir, FirFilter};
pub use gain::{GainRamp, LinearGain, MAX_LINEAR_GAIN};
pub use orbit::{DirectionBlend, HorizontalStereoPair, blend_for_azimuth, normalize_azimuth};
pub use preset::{PreparedHrir, PreparedHrirPair};
pub use processor::AudioProcessor;
pub use render::{FixedFrontRenderer, HorizontalOrbitRenderer, StereoRenderer};
pub use renderer_state::{PreparedRenderer, RenderMode};
pub use resample::OUTPUT_SAMPLE_RATE_HZ;
pub use signal::{MAX_ABS_INPUT_SAMPLE, MAX_ABS_OUTPUT_SAMPLE};
pub use transition::equal_power_weights;
