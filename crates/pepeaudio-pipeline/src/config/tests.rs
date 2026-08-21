use std::time::Duration;

use super::PipelineConfig;
use crate::PipelineError;

#[test]
fn defaults_are_valid_and_frame_aligned() {
    let config = PipelineConfig::default();
    assert!(config.validate().is_ok());
}

#[test]
fn rejects_resource_exhaustion_configuration() {
    for config in [
        PipelineConfig {
            block_frames: usize::MAX,
            ..PipelineConfig::default()
        },
        PipelineConfig {
            pcm_buffer_bytes: usize::MAX,
            ..PipelineConfig::default()
        },
        PipelineConfig {
            event_capacity: usize::MAX,
            ..PipelineConfig::default()
        },
        PipelineConfig {
            shutdown_timeout: Duration::MAX,
            ..PipelineConfig::default()
        },
    ] {
        assert!(matches!(
            config.validate(),
            Err(PipelineError::InvalidConfig)
        ));
    }
}
