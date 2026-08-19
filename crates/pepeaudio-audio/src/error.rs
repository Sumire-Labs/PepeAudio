use pepeaudio_hrir::VirtualDirection;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Ear {
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq, Error)]
pub enum DspError {
    #[error("an impulse response must contain at least one sample")]
    EmptyImpulse,
    #[error("impulse response sample {index} is not finite")]
    NonFiniteImpulse { index: usize },
    #[error("impulse response sample {index} has magnitude {actual}, above {maximum}")]
    ImpulseSampleTooLarge {
        index: usize,
        actual: f32,
        maximum: f32,
    },
    #[error("impulse response absolute gain {actual} is above {maximum}")]
    ImpulseGainTooLarge { actual: f64, maximum: f64 },
    #[error("prepared {direction:?} {ear:?} plane has {actual} frames; expected {expected}")]
    PreparedPlaneLength {
        direction: VirtualDirection,
        ear: Ear,
        actual: usize,
        expected: usize,
    },
    #[error("resampled impulse response length overflowed usize")]
    ResampleLengthOverflow,
    #[error("partitioned convolution backend rejected a prepared operation")]
    ConvolutionBackend,
    #[error("input block has {input} samples but output block has {output}")]
    BlockLengthMismatch { input: usize, output: usize },
    #[error("stereo block contains an odd sample count: {samples}")]
    OddStereoBlock { samples: usize },
    #[error("maximum block size must be at least one frame")]
    ZeroBlockCapacity,
    #[error("block contains {actual} frames; processor capacity is {maximum}")]
    BlockTooLarge { actual: usize, maximum: usize },
    #[error("{samples} samples cannot be interpreted as frames of {channels} channels")]
    InvalidInterleavedBlock { samples: usize, channels: usize },
    #[error("input sample {index} is not finite")]
    NonFiniteInput { index: usize },
    #[error("input sample {index} has magnitude {actual}, above {maximum}")]
    InputSampleTooLarge {
        index: usize,
        actual: f32,
        maximum: f32,
    },
    #[error("DSP output sample {index} is not finite")]
    NonFiniteOutput { index: usize },
    #[error("linear gain {actual} is outside 0..={maximum}")]
    InvalidGain { actual: f32, maximum: f32 },
    #[error("transition progress {actual} is not finite")]
    InvalidTransitionProgress { actual: f32 },
    #[error("azimuth {actual} degrees is not finite")]
    InvalidAzimuth { actual: f32 },
    #[error("stereo width {actual} degrees is outside 0..=180")]
    InvalidStereoWidth { actual: f32 },
    #[error("horizontal orbit controls require horizontal-orbit render mode")]
    OrbitModeRequired,
    #[error("prepared renderer mode does not match the active processor mode")]
    RendererModeMismatch,
    #[error("a preset transition is already in progress")]
    PresetTransitionInProgress,
}
