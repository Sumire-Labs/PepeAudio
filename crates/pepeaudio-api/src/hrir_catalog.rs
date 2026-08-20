use pepeaudio_core::{GuildId, HrirPresetId};
use serde::Serialize;

/// Authenticated, guild-scoped HRIR catalog response.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct HrirPresetCatalog {
    pub guild_id: GuildId,
    /// Ordered global and guild-owned presets visible to this guild.
    pub presets: Vec<HrirPresetSummary>,
}

/// Public selector data for one installed HRIR preset.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct HrirPresetSummary {
    /// Stable identifier accepted by `set_hrir` commands.
    pub id: HrirPresetId,
    pub display_name: String,
    /// Optional short explanation displayed with the selector option.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Optional attribution and licensing data safe for public display.
    pub source: HrirSourceMetadata,
}

/// Public source metadata; storage paths and integrity internals are excluded.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct HrirSourceMetadata {
    /// SPDX identifier or human-readable license name, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license_name: Option<String>,
    /// Canonical public license or source URL, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    /// Attribution required by the source, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attribution: Option<String>,
}
