use std::time::Duration;

use pepeaudio_audio::{HorizontalStereoPair, OUTPUT_SAMPLE_RATE_HZ};

use crate::{PipelineError, PipelineResult};

const FULL_CIRCLE_DEGREES: f64 = 360.0;

/// Sample-clocked horizontal movement for one PCM generation.
///
/// Positive azimuth is counter-clockwise in `pepeaudio-audio`, so subtracting
/// phase produces clockwise movement. Only processed PCM frames advance the
/// clock; elapsed wall time never does.
#[derive(Clone, Copy, Debug)]
pub(crate) struct OrbitClock {
    period_frames: u64,
    phase_frames: u64,
    origin: HorizontalStereoPair,
}

impl OrbitClock {
    pub(crate) fn new(
        period: Duration,
        track_position: Duration,
        origin: HorizontalStereoPair,
    ) -> PipelineResult<Self> {
        let period_frames = duration_frames(period)?;
        let phase_frames = duration_frames_modulo(track_position, period_frames)?;
        Ok(Self {
            period_frames,
            phase_frames,
            origin,
        })
    }

    // PipelineConfig caps a period at ten minutes (28.8 million frames), so
    // both operands and the resulting normalized angle are exactly bounded
    // before the renderer's required f32 representation.
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
    pub(crate) fn position(self) -> PipelineResult<HorizontalStereoPair> {
        let progress = self.phase_frames as f64 / self.period_frames as f64;
        let center = f64::from(self.origin.center_degrees()) - FULL_CIRCLE_DEGREES * progress;
        HorizontalStereoPair::new(center as f32, self.origin.width_degrees()).map_err(Into::into)
    }

    pub(crate) fn advance(&mut self, frames: usize) -> PipelineResult<()> {
        let frames = u64::try_from(frames).map_err(|_| PipelineError::InvalidConfig)?;
        self.phase_frames = self
            .phase_frames
            .checked_add(frames % self.period_frames)
            .ok_or(PipelineError::InvalidConfig)?
            % self.period_frames;
        Ok(())
    }

    pub(crate) fn rebase(&mut self, position: HorizontalStereoPair) {
        self.origin = position;
        self.phase_frames = 0;
    }
}

fn duration_frames(duration: Duration) -> PipelineResult<u64> {
    let frames =
        duration.as_nanos() * u128::from(OUTPUT_SAMPLE_RATE_HZ) / Duration::from_secs(1).as_nanos();
    let frames = u64::try_from(frames).map_err(|_| PipelineError::InvalidConfig)?;
    if frames == 0 {
        return Err(PipelineError::InvalidConfig);
    }
    Ok(frames)
}

fn duration_frames_modulo(duration: Duration, period_frames: u64) -> PipelineResult<u64> {
    let frames =
        duration.as_nanos() * u128::from(OUTPUT_SAMPLE_RATE_HZ) / Duration::from_secs(1).as_nanos();
    u64::try_from(frames % u128::from(period_frames)).map_err(|_| PipelineError::InvalidConfig)
}

#[cfg(test)]
mod tests {
    use super::OrbitClock;
    use pepeaudio_audio::HorizontalStereoPair;
    use std::time::Duration;

    #[test]
    fn sample_clock_moves_clockwise_and_wraps_without_wall_time() {
        let mut clock = OrbitClock::new(
            Duration::from_mins(1),
            Duration::ZERO,
            HorizontalStereoPair::FRONT,
        )
        .expect("clock");

        assert_center(clock, 0.0);
        assert_center(clock, 0.0);
        clock.advance(48_000 * 15).expect("advance");
        assert_center(clock, -90.0);
        clock.advance(48_000 * 45).expect("advance");
        assert_center(clock, 0.0);
    }

    #[test]
    fn seek_position_selects_deterministic_track_phase() {
        let clock = OrbitClock::new(
            Duration::from_mins(1),
            Duration::from_secs(45),
            HorizontalStereoPair::FRONT,
        )
        .expect("clock");

        assert_center(clock, 90.0);
    }

    fn assert_center(clock: OrbitClock, expected: f32) {
        let actual = clock.position().expect("position").center_degrees();
        assert!(
            (actual - expected).abs() < 0.000_1,
            "{actual} != {expected}"
        );
    }
}
