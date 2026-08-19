use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use pepeaudio_audio::PreparedHrir;
use pepeaudio_core::HrirPresetId;
use tokio::sync::RwLock;

use crate::{PipelineError, PipelineResult};

/// Prepared-HRIR lookup boundary used by `PlaybackPort::set_hrir`.
#[async_trait]
pub trait HrirProvider: Send + Sync {
    /// Returns a fully prepared 48 kHz preset without filesystem access in the
    /// PCM processing loop.
    async fn get(&self, id: &HrirPresetId) -> PipelineResult<Arc<PreparedHrir>>;
}

/// Adapts an in-memory synchronous lookup, such as a preset catalog, without
/// requiring the catalog's crate to depend on this playback crate.
pub struct LookupHrirProvider<F> {
    lookup: F,
}

impl<F> LookupHrirProvider<F> {
    #[must_use]
    pub const fn new(lookup: F) -> Self {
        Self { lookup }
    }
}

#[async_trait]
impl<F> HrirProvider for LookupHrirProvider<F>
where
    F: Fn(&HrirPresetId) -> Option<Arc<PreparedHrir>> + Send + Sync,
{
    async fn get(&self, id: &HrirPresetId) -> PipelineResult<Arc<PreparedHrir>> {
        (self.lookup)(id).ok_or(PipelineError::HrirNotFound)
    }
}

/// Concurrent in-memory registry populated by startup/import control paths.
#[derive(Debug, Default)]
pub struct InMemoryHrirProvider {
    presets: RwLock<HashMap<HrirPresetId, Arc<PreparedHrir>>>,
}

impl InMemoryHrirProvider {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn insert(&self, id: HrirPresetId, preset: Arc<PreparedHrir>) {
        self.presets.write().await.insert(id, preset);
    }

    pub async fn remove(&self, id: &HrirPresetId) -> Option<Arc<PreparedHrir>> {
        self.presets.write().await.remove(id)
    }
}

#[async_trait]
impl HrirProvider for InMemoryHrirProvider {
    async fn get(&self, id: &HrirPresetId) -> PipelineResult<Arc<PreparedHrir>> {
        self.presets
            .read()
            .await
            .get(id)
            .cloned()
            .ok_or(PipelineError::HrirNotFound)
    }
}
