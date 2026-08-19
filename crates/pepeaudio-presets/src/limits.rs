use std::num::{NonZeroU64, NonZeroUsize};

use crate::{CatalogError, CatalogResult};

/// Resource bounds applied while discovering and preparing preset assets.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CatalogLimits {
    preset_capacity: NonZeroUsize,
    file_size_limit: NonZeroU64,
    frame_limit: NonZeroUsize,
    prepared_frame_limit: Option<NonZeroUsize>,
}

impl CatalogLimits {
    /// # Errors
    ///
    /// Returns an error when any limit is zero.
    pub fn new(max_presets: usize, max_file_bytes: u64, max_frames: usize) -> CatalogResult<Self> {
        Ok(Self {
            preset_capacity: NonZeroUsize::new(max_presets)
                .ok_or(CatalogError::InvalidLimit("max presets"))?,
            file_size_limit: NonZeroU64::new(max_file_bytes)
                .ok_or(CatalogError::InvalidLimit("max file bytes"))?,
            frame_limit: NonZeroUsize::new(max_frames)
                .ok_or(CatalogError::InvalidLimit("max frames"))?,
            prepared_frame_limit: None,
        })
    }

    /// This is distinct from the source WAV allocation bound: a 44.1 kHz
    /// preset grows by 160/147 during preparation.
    ///
    /// # Errors
    ///
    /// Returns an error when `maximum` is zero.
    pub fn with_prepared_frame_limit(mut self, maximum: usize) -> CatalogResult<Self> {
        self.prepared_frame_limit = Some(
            NonZeroUsize::new(maximum).ok_or(CatalogError::InvalidLimit("prepared max frames"))?,
        );
        Ok(self)
    }

    pub(crate) const fn max_presets(self) -> usize {
        self.preset_capacity.get()
    }

    pub(crate) const fn max_file_bytes(self) -> u64 {
        self.file_size_limit.get()
    }

    pub(crate) const fn max_frames(self) -> usize {
        self.frame_limit.get()
    }

    pub(crate) const fn max_prepared_frames(self) -> Option<usize> {
        match self.prepared_frame_limit {
            Some(limit) => Some(limit.get()),
            None => None,
        }
    }
}

impl Default for CatalogLimits {
    fn default() -> Self {
        Self {
            preset_capacity: NonZeroUsize::new(256).expect("constant is non-zero"),
            file_size_limit: NonZeroU64::new(64 * 1024 * 1024).expect("constant is non-zero"),
            frame_limit: NonZeroUsize::new(pepeaudio_hrir::DEFAULT_MAX_FRAMES)
                .expect("constant is non-zero"),
            prepared_frame_limit: None,
        }
    }
}
