//! Safe loading and normalization of `HeSuVi` HRIR WAVE files.
//!
//! This crate stops at parsing and structural normalization. It intentionally
//! does not resample, convolve, spatialize, hash, identify, or store presets.
//! A path passed to [`load_hesuvi_wav_file`] is only an I/O locator and is never
//! used as preset identity.

#![forbid(unsafe_code)]

mod error;
mod loader;
mod model;

pub use error::{LoadError, WaveSampleKind};
pub use loader::{
    DEFAULT_MAX_FRAMES, LoadLimits, load_hesuvi_wav, load_hesuvi_wav_file,
    load_hesuvi_wav_file_with_limits, load_hesuvi_wav_with_limits,
};
pub use model::{
    ALL_DIRECTIONS, HesuviPreset, HesuviSampleRate, HrirPair, SourceLayout, VirtualDirection,
};
