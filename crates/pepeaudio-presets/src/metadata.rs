use std::{collections::BTreeMap, path::Path};

use pepeaudio_core::HrirPresetId;

use crate::{CatalogError, CatalogResult};

const METADATA_FILE_NAME: &str = "info.csv";
const MAX_METADATA_BYTES: u64 = 64 * 1024;
const MAX_METADATA_ENTRIES: usize = 512;
pub(crate) const MAX_DISPLAY_NAME_CHARS: usize = 100;
pub(crate) const MAX_DESCRIPTION_CHARS: usize = 240;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PresetPresentation {
    pub(crate) display_name: String,
    pub(crate) description: Option<String>,
}

pub(crate) fn load(root: &Path) -> CatalogResult<BTreeMap<HrirPresetId, PresetPresentation>> {
    let path = root.join(METADATA_FILE_NAME);
    let metadata = match std::fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(error) => return Err(CatalogError::Filesystem(error)),
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(CatalogError::UnsafeMetadata);
    }
    if metadata.len() > MAX_METADATA_BYTES {
        return Err(CatalogError::MetadataTooLarge {
            maximum: MAX_METADATA_BYTES,
        });
    }

    let canonical = path.canonicalize().map_err(CatalogError::Filesystem)?;
    if canonical.parent() != Some(root) {
        return Err(CatalogError::UnsafeMetadata);
    }
    let bytes = std::fs::read(canonical).map_err(CatalogError::Filesystem)?;
    if u64::try_from(bytes.len()).ok() != Some(metadata.len()) {
        return Err(CatalogError::UnsafeMetadata);
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| CatalogError::InvalidMetadata {
        line: 1,
        reason: "file must be UTF-8",
    })?;
    parse(text.strip_prefix('\u{feff}').unwrap_or(text))
}

fn parse(text: &str) -> CatalogResult<BTreeMap<HrirPresetId, PresetPresentation>> {
    let mut presentations = BTreeMap::new();
    for (index, line) in text.lines().enumerate() {
        let line_number = index + 1;
        if line.is_empty() {
            continue;
        }
        if presentations.len() == MAX_METADATA_ENTRIES {
            return Err(CatalogError::InvalidMetadata {
                line: line_number,
                reason: "file contains too many entries",
            });
        }
        let (raw_id, raw_information) =
            line.split_once(';').ok_or(CatalogError::InvalidMetadata {
                line: line_number,
                reason: "expected an ID and description separated by a semicolon",
            })?;
        if raw_id == "*" {
            continue;
        }
        let preset_id = HrirPresetId::new(raw_id).map_err(|_| CatalogError::InvalidMetadata {
            line: line_number,
            reason: "preset ID is invalid",
        })?;
        let presentation = presentation(&preset_id, raw_information, line_number)?;
        if presentations
            .insert(preset_id.clone(), presentation)
            .is_some()
        {
            return Err(CatalogError::DuplicateMetadata {
                preset_id: preset_id.to_string(),
            });
        }
    }
    Ok(presentations)
}

fn presentation(
    preset_id: &HrirPresetId,
    information: &str,
    line: usize,
) -> CatalogResult<PresetPresentation> {
    if information.trim() != information
        || information.is_empty()
        || information.chars().any(char::is_control)
    {
        return Err(CatalogError::InvalidMetadata {
            line,
            reason: "description must be non-empty canonical text",
        });
    }

    if let Some(curated) = curated_presentation(preset_id.as_str()) {
        return Ok(curated);
    }

    let mut paragraphs = information.split("/n/n");
    let primary = paragraphs.next().unwrap_or_default().trim();
    if primary.is_empty() {
        return Err(CatalogError::InvalidMetadata {
            line,
            reason: "display name must not be empty",
        });
    }
    let extra = paragraphs
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let display_name = abbreviate(primary, MAX_DISPLAY_NAME_CHARS);
    let description = if display_name == primary && extra.is_empty() {
        None
    } else {
        let detail = if display_name == primary {
            extra
        } else if extra.is_empty() {
            primary.to_owned()
        } else {
            format!("{primary} {extra}")
        };
        Some(abbreviate(&detail, MAX_DESCRIPTION_CHARS))
    };

    Ok(PresetPresentation {
        display_name,
        description,
    })
}

fn curated_presentation(preset_id: &str) -> Option<PresetPresentation> {
    let (display_name, description) = match preset_id {
        "atmos-" => (
            "Dolby Atmos 7.1 (No Reverb)",
            Some("Virtual surround sound for headphones without reverb."),
        ),
        "cmss_game" => (
            "CMSS-3D — Game Mode",
            Some("Recorded on X-Fi Titanium by Sossaman."),
        ),
        "dht" => ("Aura Cinema 4.1", Some("Headphone Surround Virtualizer.")),
        "ssc_ny" => (
            "Spatial Sound Card — New York",
            Some("Short room envelope. Do not use any upmix."),
        ),
        "waves" => ("Waves NX", None),
        _ => return None,
    };
    Some(PresetPresentation {
        display_name: display_name.to_owned(),
        description: description.map(str::to_owned),
    })
}

fn abbreviate(value: &str, maximum: usize) -> String {
    if value.chars().count() <= maximum {
        return value.to_owned();
    }

    let mut prefix = value.chars().take(maximum - 1).collect::<String>();
    if let Some(word_boundary) = prefix.rfind(char::is_whitespace) {
        prefix.truncate(word_boundary);
    }
    prefix.push('…');
    prefix
}

#[cfg(test)]
mod tests {
    use super::{MAX_DISPLAY_NAME_CHARS, parse};

    #[test]
    fn extracts_hesuvi_warning_as_secondary_description() {
        let entries = parse(
            "custom;Spatial Sound Card with New York location/n/nCAUTION: Do not use any upmix!\n",
        )
        .expect("metadata");
        let item = entries.values().next().expect("entry");

        assert_eq!(
            item.display_name,
            "Spatial Sound Card with New York location"
        );
        assert_eq!(
            item.description.as_deref(),
            Some("CAUTION: Do not use any upmix!")
        );
    }

    #[test]
    fn uses_verified_names_for_known_hesuvi_presets() {
        let entries = parse(
            "atmos-;Dolby Atmos 7.1 virtual surround sound for headphones without reverb\n\
             dht;Dolby Home Theater v4 Headphone Surround Virtualizer\n",
        )
        .expect("metadata");

        assert_eq!(
            entries
                .values()
                .map(|item| (item.display_name.as_str(), item.description.as_deref()))
                .collect::<Vec<_>>(),
            vec![
                (
                    "Dolby Atmos 7.1 (No Reverb)",
                    Some("Virtual surround sound for headphones without reverb.")
                ),
                ("Aura Cinema 4.1", Some("Headphone Surround Virtualizer."))
            ]
        );
    }

    #[test]
    fn abbreviates_long_labels_without_losing_the_original_information() {
        let information = format!("{} tail", "Japanese-safe ".repeat(12));
        let entries = parse(&format!("long;{information}\n")).expect("metadata");
        let item = entries.values().next().expect("entry");

        assert!(item.display_name.chars().count() <= MAX_DISPLAY_NAME_CHARS);
        assert!(item.display_name.ends_with('…'));
        assert_eq!(item.description.as_deref(), Some(information.as_str()));
    }

    #[test]
    fn rejects_duplicate_ids_without_exposing_metadata_text() {
        let error = parse("same;First\nsame;Second\n").expect_err("duplicate");
        assert!(matches!(
            error,
            crate::CatalogError::DuplicateMetadata { preset_id } if preset_id == "same"
        ));
    }
}
