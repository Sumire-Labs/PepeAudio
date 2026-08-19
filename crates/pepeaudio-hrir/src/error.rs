use std::fmt;

use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaveSampleKind {
    Integer,
    Float,
}

impl fmt::Display for WaveSampleKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Integer => formatter.write_str("integer PCM"),
            Self::Float => formatter.write_str("IEEE float"),
        }
    }
}

#[derive(Debug, Error)]
pub enum LoadError {
    #[error("failed to read WAVE data: {0}")]
    Wave(#[from] hound::Error),

    #[error("unsupported channel count {actual}; expected 7 or 14")]
    UnsupportedChannelCount {
        /// Channel count found in the WAVE header.
        actual: u16,
    },

    #[error("unsupported sample rate {actual} Hz; expected 44100 or 48000 Hz")]
    UnsupportedSampleRate {
        /// Sample rate found in the WAVE header.
        actual: u32,
    },

    #[error(
        "unsupported {kind} sample encoding with {bits_per_sample} bits; expected PCM16 or f32"
    )]
    UnsupportedSampleEncoding {
        kind: WaveSampleKind,
        bits_per_sample: u16,
    },

    #[error("the HRIR contains zero frames")]
    ZeroLength,

    #[error("the HRIR contains {actual} frames, exceeding the configured maximum of {maximum}")]
    TooManyFrames {
        /// Per-channel frame count found in the WAVE data.
        actual: usize,
        /// Configured maximum per-channel frame count.
        maximum: usize,
    },

    #[error("the HRIR frame count {actual} cannot be represented on this platform")]
    FrameCountOutOfRange {
        /// Per-channel frame count reported by the WAVE reader.
        actual: u64,
    },

    #[error("non-finite sample at frame {frame}, channel {channel}")]
    NonFiniteSample {
        /// Zero-based frame index.
        frame: usize,
        /// Zero-based source channel index.
        channel: usize,
    },

    #[error("source channel {channel} has {actual} frames; expected {expected}")]
    UnequalFrameCount {
        channel: usize,
        expected: usize,
        actual: usize,
    },
}
