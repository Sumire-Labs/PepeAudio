use std::io;

use pepeaudio_audio::DspError;

pub type PipelineResult<T> = Result<T, PipelineError>;

#[derive(Debug, thiserror::Error)]
pub enum PipelineError {
    #[error("pipeline configuration is invalid")]
    InvalidConfig,
    #[error("guild is not connected to a voice channel")]
    NotConnected,
    #[error("guild has no active playback track")]
    NoActiveTrack,
    #[error("playback source is not a managed regular file")]
    InvalidSource,
    #[error("managed media root is unavailable")]
    ManagedRoot(#[source] io::Error),
    #[error("managed media object is unavailable")]
    Resolve(#[source] io::Error),
    #[error("selected HRIR preset is unavailable")]
    HrirNotFound,
    #[error("FFmpeg decoder could not be spawned")]
    DecoderSpawn(#[source] io::Error),
    #[error("FFmpeg decoder pipe failed")]
    DecoderPipe(#[source] io::Error),
    #[error("FFmpeg decoder lifecycle operation failed")]
    DecoderLifecycle(#[source] io::Error),
    #[error("FFmpeg decoder exited unsuccessfully with code {code:?}")]
    DecoderExit { code: Option<i32> },
    #[error("FFmpeg decoder diagnostics exceeded their configured limit")]
    DecoderDiagnosticsTooLarge,
    #[error("pipeline operation timed out")]
    Timeout,
    #[error("FFmpeg output ended with a partial PCM frame")]
    PartialPcmFrame,
    #[error("audio DSP rejected pipeline data")]
    Dsp(#[from] DspError),
    #[error("audio DSP worker is unavailable")]
    WorkerClosed,
    #[error("audio DSP control update was rejected")]
    DspControl,
    #[error("Songbird voice operation failed")]
    Voice,
    #[error("Songbird track control failed")]
    TrackControl,
    #[error("pipeline worker task failed")]
    WorkerTask,
    #[error("Songbird PCM input closed unexpectedly")]
    OutputClosed,
}
