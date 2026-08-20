use std::{collections::BTreeMap, io::Cursor, path::Path, sync::Arc};

use pepeaudio_audio::PreparedHrir;
use pepeaudio_core::HrirPresetId;
use pepeaudio_hrir::{HesuviSampleRate, LoadLimits, SourceLayout};
use sha2::{Digest as _, Sha256};

use crate::{
    CatalogError, CatalogLimits, CatalogResult,
    metadata::{MAX_DISPLAY_NAME_CHARS, PresetPresentation},
};

/// Public metadata for one prepared operator-installed preset.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HrirDescriptor {
    /// Stable ID derived from the direct WAV filename stem.
    pub id: HrirPresetId,
    pub display_name: String,
    /// Optional secondary text supplied by the operator's `info.csv`.
    pub description: Option<String>,
    pub source_sample_rate_hz: u32,
    pub source_layout: SourceLayout,
    pub prepared_frames: usize,
    /// Direct filename used as the operator-managed storage key.
    pub storage_key: String,
    pub sha256_hex: String,
    pub file_size_bytes: u64,
}

#[derive(Clone, Debug)]
struct CatalogEntry {
    descriptor: HrirDescriptor,
    prepared: Arc<PreparedHrir>,
}

/// Immutable in-memory catalog loaded entirely before gateway startup.
#[derive(Clone, Debug, Default)]
pub struct HrirCatalog {
    entries: Arc<BTreeMap<HrirPresetId, CatalogEntry>>,
}

impl HrirCatalog {
    /// Discovers direct `.wav` children, validates `HeSuVi` layout, and prepares
    /// every usable asset at 48 kHz.
    ///
    /// Non-WAV files and subdirectories are ignored so attribution documents
    /// may live beside the presets. WAV symlinks are rejected.
    ///
    /// # Errors
    ///
    /// Returns before producing a catalog when any candidate is unsafe,
    /// malformed, duplicated, or outside the configured resource limits.
    pub fn load(root: impl AsRef<Path>, limits: CatalogLimits) -> CatalogResult<Self> {
        let root = root
            .as_ref()
            .canonicalize()
            .map_err(|_| CatalogError::InvalidRoot)?;
        if !root.is_dir() {
            return Err(CatalogError::InvalidRoot);
        }

        let presentations = crate::metadata::load(&root)?;
        let mut entries = BTreeMap::new();
        for candidate in discover_candidates(&root, limits)? {
            let entry = load_entry(&root, &candidate, limits, &presentations)?;
            let id = entry.descriptor.id.clone();
            if entries.insert(id.clone(), entry).is_some() {
                return Err(CatalogError::DuplicateId {
                    preset_id: id.to_string(),
                });
            }
        }

        Ok(Self {
            entries: Arc::new(entries),
        })
    }

    /// Returns immutable 48 kHz coefficients without filesystem work.
    #[must_use]
    pub fn get(&self, id: &HrirPresetId) -> Option<Arc<PreparedHrir>> {
        self.entries
            .get(id)
            .map(|entry| Arc::clone(&entry.prepared))
    }

    /// Returns descriptors in stable preset-ID order.
    #[must_use]
    pub fn descriptors(&self) -> Vec<HrirDescriptor> {
        self.entries
            .values()
            .map(|entry| entry.descriptor.clone())
            .collect()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

fn discover_candidates(
    root: &Path,
    limits: CatalogLimits,
) -> CatalogResult<Vec<std::fs::DirEntry>> {
    let mut candidates = Vec::new();
    for entry in std::fs::read_dir(root).map_err(CatalogError::Filesystem)? {
        let entry = entry.map_err(CatalogError::Filesystem)?;
        if !is_wav(&entry.path()) {
            continue;
        }
        if candidates.len() == limits.max_presets() {
            return Err(CatalogError::TooManyPresets {
                maximum: limits.max_presets(),
            });
        }
        candidates.push(entry);
    }
    candidates.sort_by_key(std::fs::DirEntry::file_name);
    Ok(candidates)
}

fn load_entry(
    root: &Path,
    candidate: &std::fs::DirEntry,
    limits: CatalogLimits,
    presentations: &BTreeMap<HrirPresetId, PresetPresentation>,
) -> CatalogResult<CatalogEntry> {
    let file_name = candidate.file_name().to_string_lossy().into_owned();
    let file_type = candidate.file_type().map_err(CatalogError::Filesystem)?;
    if file_type.is_symlink() || !file_type.is_file() {
        return Err(CatalogError::UnsafeEntry { file_name });
    }
    let canonical = candidate
        .path()
        .canonicalize()
        .map_err(CatalogError::Filesystem)?;
    if canonical.parent() != Some(root) {
        return Err(CatalogError::UnsafeEntry { file_name });
    }
    let metadata = candidate.metadata().map_err(CatalogError::Filesystem)?;
    if metadata.len() > limits.max_file_bytes() {
        return Err(CatalogError::FileTooLarge {
            file_name,
            maximum: limits.max_file_bytes(),
        });
    }
    let id =
        HrirPresetId::new(file_stem(candidate)?).map_err(|_| CatalogError::InvalidIdentifier {
            reason: "filename stem violates the canonical preset ID rules",
        })?;
    let presentation = presentations.get(&id);
    let display_name =
        presentation.map_or_else(|| id.to_string(), |metadata| metadata.display_name.clone());
    let description = presentation.and_then(|metadata| metadata.description.clone());
    let source_bytes = std::fs::read(&canonical).map_err(CatalogError::Filesystem)?;
    let file_size_bytes =
        u64::try_from(source_bytes.len()).map_err(|_| CatalogError::FileTooLarge {
            file_name: file_name.clone(),
            maximum: limits.max_file_bytes(),
        })?;
    if file_size_bytes != metadata.len() || file_size_bytes > limits.max_file_bytes() {
        return Err(CatalogError::UnsafeEntry { file_name });
    }
    let sha256_hex = format!("{:x}", Sha256::digest(&source_bytes));
    let hesuvi = pepeaudio_hrir::load_hesuvi_wav_with_limits(
        Cursor::new(source_bytes),
        LoadLimits::new(limits.max_frames()),
    )
    .map_err(|source| CatalogError::InvalidHesuvi {
        file_name: file_name.clone(),
        source,
    })?;
    let prepared =
        PreparedHrir::from_hesuvi(&hesuvi).map_err(|source| CatalogError::InvalidDsp {
            file_name: file_name.clone(),
            source,
        })?;
    if let Some(maximum) = limits.max_prepared_frames()
        && prepared.frame_count() > maximum
    {
        return Err(CatalogError::PreparedFramesTooLarge {
            file_name,
            actual: prepared.frame_count(),
            maximum,
        });
    }
    Ok(CatalogEntry {
        descriptor: HrirDescriptor {
            id,
            display_name,
            description,
            source_sample_rate_hz: sample_rate_hz(hesuvi.sample_rate()),
            source_layout: hesuvi.source_layout(),
            prepared_frames: prepared.frame_count(),
            storage_key: file_name,
            sha256_hex,
            file_size_bytes,
        },
        prepared: Arc::new(prepared),
    })
}

fn file_stem(candidate: &std::fs::DirEntry) -> CatalogResult<String> {
    let stem = candidate
        .path()
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or(CatalogError::InvalidIdentifier {
            reason: "filename stem must be valid UTF-8",
        })?
        .to_owned();
    if stem.chars().count() > MAX_DISPLAY_NAME_CHARS {
        Err(CatalogError::InvalidIdentifier {
            reason: "filename stem must fit a 100-character Discord option",
        })
    } else {
        Ok(stem)
    }
}

fn is_wav(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("wav"))
}

const fn sample_rate_hz(rate: HesuviSampleRate) -> u32 {
    rate.as_hz()
}
