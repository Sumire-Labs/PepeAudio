use std::time::Duration;

use crate::{PipelineError, PipelineResult};

const PCM_FRAME_BYTES: usize = 2 * size_of::<f32>();
const MAX_BLOCK_FRAMES: usize = 4_800;
const MAX_PCM_BUFFER_BYTES: usize = 1_536_000;
const MAX_SONGBIRD_BUFFER_BYTES: usize = 384_000;
const MAX_CHANNEL_CAPACITY: usize = 4_096;
const MAX_TRANSITION_FRAMES: usize = 96_000;
const MAX_SHUTDOWN_TIMEOUT: Duration = Duration::from_mins(1);
const MIN_ORBIT_PERIOD: Duration = Duration::from_secs(1);
const MAX_ORBIT_PERIOD: Duration = Duration::from_mins(10);

/// Bounded buffering and DSP transition policy for one guild pipeline.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PipelineConfig {
    /// Frames processed in one allocation-free DSP block.
    pub block_frames: usize,
    /// Bytes between the DSP producer and Songbird's async adapter.
    pub pcm_buffer_bytes: usize,
    /// Bytes in Songbird's async-to-sync ring buffer.
    pub songbird_buffer_bytes: usize,
    pub control_capacity: usize,
    pub event_capacity: usize,
    /// Equal-power HRIR and wet/gain ramp length.
    pub transition_frames: usize,
    pub orbit_period: Duration,
    pub shutdown_timeout: Duration,
}

impl PipelineConfig {
    /// # Errors
    ///
    /// Returns [`PipelineError::InvalidConfig`] for a zero, misaligned, or
    /// overflowing bound.
    pub fn validate(self) -> PipelineResult<Self> {
        if self.block_frames == 0
            || self.block_frames > MAX_BLOCK_FRAMES
            || self.control_capacity == 0
            || self.control_capacity > MAX_CHANNEL_CAPACITY
            || self.event_capacity == 0
            || self.event_capacity > MAX_CHANNEL_CAPACITY
            || self.transition_frames > MAX_TRANSITION_FRAMES
            || self.orbit_period < MIN_ORBIT_PERIOD
            || self.orbit_period > MAX_ORBIT_PERIOD
            || self.shutdown_timeout.is_zero()
            || self.shutdown_timeout > MAX_SHUTDOWN_TIMEOUT
            || self.pcm_buffer_bytes > MAX_PCM_BUFFER_BYTES
            || self.songbird_buffer_bytes > MAX_SONGBIRD_BUFFER_BYTES
        {
            return Err(PipelineError::InvalidConfig);
        }
        let block_bytes = self
            .block_frames
            .checked_mul(PCM_FRAME_BYTES)
            .ok_or(PipelineError::InvalidConfig)?;
        if self.pcm_buffer_bytes < block_bytes
            || self.songbird_buffer_bytes < PCM_FRAME_BYTES
            || !self.pcm_buffer_bytes.is_multiple_of(PCM_FRAME_BYTES)
            || !self.songbird_buffer_bytes.is_multiple_of(PCM_FRAME_BYTES)
        {
            return Err(PipelineError::InvalidConfig);
        }
        Ok(self)
    }
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            block_frames: 960,
            pcm_buffer_bytes: 96_000,
            songbird_buffer_bytes: 32_000,
            control_capacity: 16,
            event_capacity: 64,
            transition_frames: 2_400,
            orbit_period: Duration::from_mins(1),
            shutdown_timeout: Duration::from_secs(10),
        }
    }
}

#[cfg(test)]
mod tests;
