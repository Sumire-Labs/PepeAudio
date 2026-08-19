use std::{
    fmt,
    sync::{Arc, Mutex},
    time::Duration,
};

use async_trait::async_trait;

use crate::{PipelineResult, ResolvedSource};

/// One logical slot in a decoder factory's stable process budget.
///
/// A replacement decoder may share the active track's slot while a separate
/// permit accounts for the brief process overlap. The slot is opaque so pool
/// details do not leak into playback.
#[derive(Clone)]
pub struct DecoderProcessSlot {
    inner: Arc<DecoderProcessSlotInner>,
}

struct DecoderProcessSlotInner {
    owner: Arc<()>,
    _permit: Mutex<Box<dyn Send + 'static>>,
}

impl DecoderProcessSlot {
    pub(crate) fn tracked(owner: Arc<()>, permit: impl Send + 'static) -> Self {
        Self {
            inner: Arc::new(DecoderProcessSlotInner {
                owner,
                _permit: Mutex::new(Box::new(permit)),
            }),
        }
    }

    pub(crate) fn belongs_to(&self, owner: &Arc<()>) -> bool {
        Arc::ptr_eq(&self.inner.owner, owner)
    }

    pub(crate) fn untracked() -> Self {
        Self::tracked(Arc::new(()), ())
    }
}

impl fmt::Debug for DecoderProcessSlot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("DecoderProcessSlot(<opaque>)")
    }
}

/// Shared guard for the single replacement decoder allowed beyond the stable
/// process limit.
#[derive(Clone)]
pub(crate) struct DecoderReplacementPermit {
    permit: Arc<Mutex<Option<Box<dyn Send + 'static>>>>,
}

impl DecoderReplacementPermit {
    pub(crate) fn tracked(permit: impl Send + 'static) -> Self {
        Self {
            permit: Arc::new(Mutex::new(Some(Box::new(permit)))),
        }
    }

    pub(crate) fn release(&self) {
        self.permit
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
    }
}

impl fmt::Debug for DecoderReplacementPermit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("DecoderReplacementPermit(<opaque>)")
    }
}

/// Decoder process plus the stable slot and optional replacement-overlap guard
/// that must live for at least as long as its worker.
pub struct SpawnedDecoder {
    decoder: Box<dyn DecodedPcm>,
    slot: DecoderProcessSlot,
    replacement_permit: Option<DecoderReplacementPermit>,
}

impl SpawnedDecoder {
    /// Wraps a decoder whose factory does not expose a shared process budget
    /// to the pipeline.
    ///
    /// This is suitable when the decoder owns any capacity guard itself.
    #[must_use]
    pub fn untracked(decoder: Box<dyn DecodedPcm>) -> Self {
        Self::stable(decoder, DecoderProcessSlot::untracked())
    }

    pub(crate) fn stable(decoder: Box<dyn DecodedPcm>, slot: DecoderProcessSlot) -> Self {
        Self {
            decoder,
            slot,
            replacement_permit: None,
        }
    }

    pub(crate) fn replacement(
        decoder: Box<dyn DecodedPcm>,
        slot: DecoderProcessSlot,
        replacement_permit: DecoderReplacementPermit,
    ) -> Self {
        Self {
            decoder,
            slot,
            replacement_permit: Some(replacement_permit),
        }
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        Box<dyn DecodedPcm>,
        DecoderProcessSlot,
        Option<DecoderReplacementPermit>,
    ) {
        (self.decoder, self.slot, self.replacement_permit)
    }
}

/// One live 48 kHz stereo interleaved `f32le` decoder.
#[async_trait]
pub trait DecodedPcm: Send {
    /// Reads PCM bytes into a non-empty buffer, returning zero only at decoder
    /// stdout EOF.
    ///
    /// # Errors
    ///
    /// Returns a configuration error for an empty output buffer, or a
    /// sanitized pipe or decoder lifecycle error.
    async fn read_pcm(&mut self, output: &mut [u8]) -> PipelineResult<usize>;

    /// Reaps a decoder which reached stdout EOF and verifies its exit status.
    ///
    /// # Errors
    ///
    /// Returns a lifecycle, exit-status, pipe, or diagnostics-limit error.
    async fn finish(&mut self) -> PipelineResult<()>;

    /// Terminates and reaps a decoder during replacement or cancellation.
    ///
    /// # Errors
    ///
    /// Returns a process termination, reap, or stderr-drain error.
    async fn shutdown(&mut self) -> PipelineResult<()>;
}

/// Creates bounded decoders from trusted managed local media objects.
#[async_trait]
pub trait DecoderFactory: Send + Sync {
    /// Starts decoding at an absolute media offset.
    ///
    /// # Errors
    ///
    /// Returns a capacity timeout, spawn, or pipe setup error.
    async fn spawn(
        &self,
        source: &ResolvedSource,
        start_offset: Duration,
    ) -> PipelineResult<SpawnedDecoder>;

    /// Starts a replacement while sharing the active decoder's stable slot.
    ///
    /// The default uses normal fresh admission. The built-in `FFmpeg` factory
    /// reserves one bounded overlap so replacement still works when its stable
    /// pool is full.
    async fn spawn_replacement(
        &self,
        source: &ResolvedSource,
        start_offset: Duration,
        _active_slot: &DecoderProcessSlot,
    ) -> PipelineResult<SpawnedDecoder> {
        self.spawn(source, start_offset).await
    }
}
