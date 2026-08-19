//! Bounded `FFmpeg`, HRIR DSP, and Songbird playback bridge.

mod cancellation;
mod config;
mod decoder;
mod dsp;
mod error;
mod event;
mod hrir;
mod orbit;
mod pcm;
mod playback;
mod resolver;
mod songbird_input;
mod track;
mod worker_cleanup;

pub use config::PipelineConfig;
pub use decoder::{
    DecodedPcm, DecoderFactory, DecoderProcessSlot, FfmpegDecoderFactory, SpawnedDecoder,
};
pub use error::{PipelineError, PipelineResult};
pub use event::{PlaybackEndReason, PlaybackEvent, WorkerFailure};
pub use hrir::{HrirProvider, InMemoryHrirProvider, LookupHrirProvider};
pub use playback::{PipelineDependencies, PlaybackStatus, SongbirdPlayback};
pub use resolver::{ManagedMediaResolver, ResolvedSource, TrackResolver};
