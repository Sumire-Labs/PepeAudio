use thiserror::Error;

pub type CatalogResult<T> = Result<T, CatalogError>;

#[derive(Debug, Error)]
pub enum CatalogError {
    #[error("catalog limit {0} must be greater than zero")]
    InvalidLimit(&'static str),
    #[error("HRIR catalog root is not a readable directory")]
    InvalidRoot,
    #[error("HRIR catalog filesystem operation failed")]
    Filesystem(#[source] std::io::Error),
    #[error("HRIR catalog contains more than {maximum} WAV presets")]
    TooManyPresets { maximum: usize },
    #[error("HRIR preset {file_name:?} is not a direct regular file")]
    UnsafeEntry { file_name: String },
    #[error("HRIR preset filename is not a valid identifier: {reason}")]
    InvalidIdentifier { reason: &'static str },
    #[error("HRIR preset {file_name:?} exceeds the {maximum}-byte limit")]
    FileTooLarge { file_name: String, maximum: u64 },
    #[error("HRIR catalog contains duplicate preset ID {preset_id}")]
    DuplicateId { preset_id: String },
    #[error("HRIR preset {file_name:?} is not a supported HeSuVi WAV")]
    InvalidHesuvi {
        file_name: String,
        #[source]
        source: pepeaudio_hrir::LoadError,
    },
    #[error("HRIR preset {file_name:?} failed DSP preparation")]
    InvalidDsp {
        file_name: String,
        #[source]
        source: pepeaudio_audio::DspError,
    },
    #[error(
        "HRIR preset {file_name:?} prepares to {actual} frames, above the {maximum}-frame realtime limit"
    )]
    PreparedFramesTooLarge {
        file_name: String,
        actual: usize,
        maximum: usize,
    },
}
