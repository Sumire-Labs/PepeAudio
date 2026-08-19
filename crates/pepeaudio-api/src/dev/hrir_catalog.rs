use pepeaudio_core::GuildId;

use crate::{
    BoxPortFuture, HrirPresetCatalog, HrirPresetCatalogSource, HrirPresetSummary, PortError,
};

pub struct StaticHrirPresetCatalog {
    presets: Vec<HrirPresetSummary>,
}

impl StaticHrirPresetCatalog {
    #[must_use]
    pub fn new(presets: impl IntoIterator<Item = HrirPresetSummary>) -> Self {
        Self {
            presets: presets.into_iter().collect(),
        }
    }
}

impl HrirPresetCatalogSource for StaticHrirPresetCatalog {
    fn hrir_presets(
        &self,
        guild_id: GuildId,
    ) -> BoxPortFuture<'_, Result<HrirPresetCatalog, PortError>> {
        Box::pin(async move {
            Ok(HrirPresetCatalog {
                guild_id,
                presets: self.presets.clone(),
            })
        })
    }
}
