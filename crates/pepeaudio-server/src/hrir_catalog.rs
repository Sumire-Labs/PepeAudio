use pepeaudio_api::{
    BoxPortFuture, HrirPresetCatalog, HrirPresetCatalogSource, HrirPresetSummary,
    HrirSourceMetadata, PortError,
};
use pepeaudio_core::GuildId;
use pepeaudio_storage::{HrirPresetRepository, PostgresStorage};

const MAX_PUBLIC_PRESETS: usize = 1_000;

pub(crate) struct PostgresHrirPresetCatalog {
    storage: PostgresStorage,
}

impl PostgresHrirPresetCatalog {
    pub(crate) const fn new(storage: PostgresStorage) -> Self {
        Self { storage }
    }
}

impl HrirPresetCatalogSource for PostgresHrirPresetCatalog {
    fn hrir_presets(
        &self,
        guild_id: GuildId,
    ) -> BoxPortFuture<'_, Result<HrirPresetCatalog, PortError>> {
        Box::pin(async move {
            let records = self
                .storage
                .list_hrir_presets(guild_id)
                .await
                .map_err(|_| PortError::Unavailable)?;
            if records.len() > MAX_PUBLIC_PRESETS {
                return Err(PortError::Internal);
            }
            let mut presets = Vec::with_capacity(records.len());
            for record in records {
                if record.owner_guild_id.is_some_and(|owner| owner != guild_id) {
                    return Err(PortError::Internal);
                }
                presets.push(HrirPresetSummary {
                    id: record.preset_id,
                    display_name: public_text(Some(record.display_name), 120)?
                        .ok_or(PortError::Internal)?,
                    description: public_text(record.description, 240)?,
                    source: public_source(
                        record.license_name,
                        record.license_url,
                        record.attribution,
                    )?,
                });
            }
            Ok(HrirPresetCatalog { guild_id, presets })
        })
    }
}

fn public_source(
    license_name: Option<String>,
    source_url: Option<String>,
    attribution: Option<String>,
) -> Result<HrirSourceMetadata, PortError> {
    Ok(HrirSourceMetadata {
        license_name: public_text(license_name, 256)?,
        source_url: public_url(source_url)?,
        attribution: public_text(attribution, 4_096)?,
    })
}

fn public_text(value: Option<String>, max_characters: usize) -> Result<Option<String>, PortError> {
    value
        .map(|text| {
            if text.is_empty()
                || text.chars().count() > max_characters
                || text.trim() != text
                || text.chars().any(char::is_control)
            {
                Err(PortError::Internal)
            } else {
                Ok(text)
            }
        })
        .transpose()
}

fn public_url(value: Option<String>) -> Result<Option<String>, PortError> {
    let Some(url) = public_text(value, 2_048)? else {
        return Ok(None);
    };
    let Some((scheme, remainder)) = url.split_once("://") else {
        return Err(PortError::Internal);
    };
    if !(scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https"))
        || remainder.is_empty()
        || remainder.starts_with('/')
        || remainder.chars().any(char::is_whitespace)
    {
        return Err(PortError::Internal);
    }
    Ok(Some(url))
}

#[cfg(test)]
mod tests {
    use super::{public_source, public_text, public_url};

    #[test]
    fn public_source_metadata_rejects_unsafe_urls_and_unbounded_text() {
        assert!(public_url(Some("https://example.test/source".into())).is_ok());
        assert!(public_url(Some("javascript://alert".into())).is_err());
        assert!(public_source(Some("x".repeat(257)), None, None).is_err());
        assert!(public_text(Some("x".repeat(240)), 240).is_ok());
        assert!(public_text(Some("x".repeat(241)), 240).is_err());
        assert!(public_text(Some("line one\nline two".into()), 240).is_err());
    }
}
