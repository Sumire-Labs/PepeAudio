use std::time::Duration;

use super::PipelineConfig;
use crate::PipelineError;

#[test]
fn defaults_are_valid_and_frame_aligned() {
    let config = PipelineConfig::default();
    assert_eq!(config.orbit_period, Duration::from_mins(1));
    assert!(config.validate().is_ok());
    assert!(
        PipelineConfig {
            orbit_period: Duration::from_secs(1),
            ..config
        }
        .validate()
        .is_ok()
    );
    assert!(
        PipelineConfig {
            orbit_period: Duration::from_mins(10),
            ..config
        }
        .validate()
        .is_ok()
    );
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
        PipelineConfig {
            orbit_period: Duration::ZERO,
            ..PipelineConfig::default()
        },
        PipelineConfig {
            orbit_period: Duration::from_mins(11),
            ..PipelineConfig::default()
        },
    ] {
        assert!(matches!(
            config.validate(),
            Err(PipelineError::InvalidConfig)
        ));
    }
}
