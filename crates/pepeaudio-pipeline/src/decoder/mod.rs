//! Bounded, restartable PCM decoder boundary.

mod factory;
mod port;
mod process;
mod spec;

#[cfg(test)]
mod tests;

pub use factory::FfmpegDecoderFactory;
pub(crate) use port::DecoderReplacementPermit;
pub use port::{DecodedPcm, DecoderFactory, DecoderProcessSlot, SpawnedDecoder};
