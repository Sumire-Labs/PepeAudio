use std::sync::Arc;

use pepeaudio_audio::{
    AudioProcessor, HorizontalStereoPair, LinearGain, PreparedHrir, PreparedRenderer, RenderMode,
};
use tokio::sync::{mpsc, oneshot};

use crate::orbit::SpatialPosition;
use crate::{PipelineError, PipelineResult};

#[derive(Clone, Debug)]
pub(crate) struct DspState {
    pub(crate) preset: Arc<PreparedHrir>,
    pub(crate) gain: LinearGain,
    pub(crate) spatial_enabled: bool,
    pub(crate) orbit_origin: HorizontalStereoPair,
}

impl DspState {
    pub(crate) fn build_processor(
        &self,
        block_frames: usize,
        startup_transition_frames: usize,
    ) -> PipelineResult<AudioProcessor> {
        let mut processor = AudioProcessor::new(
            self.preset.as_ref(),
            RenderMode::HorizontalOrbit(self.orbit_origin),
            block_frames,
        )?;
        processor.set_output_gain(LinearGain::SILENCE, 0);
        processor.set_output_gain(self.gain, startup_transition_frames);
        processor.set_wet_enabled(self.spatial_enabled, 0);
        Ok(processor)
    }
}

#[derive(Debug)]
pub(crate) enum DspMutation {
    Gain(LinearGain),
    Preset {
        renderer: PreparedRenderer,
        enable_wet: bool,
    },
    Spatial(bool),
    Orbit(HorizontalStereoPair),
}

#[derive(Debug)]
pub(crate) struct DspCommand {
    pub(crate) mutation: DspMutation,
    pub(crate) acknowledgement: oneshot::Sender<PipelineResult<()>>,
}

#[derive(Clone, Debug)]
pub(crate) struct DspController {
    sender: mpsc::Sender<DspCommand>,
}

impl DspController {
    pub(crate) fn new(sender: mpsc::Sender<DspCommand>) -> Self {
        Self { sender }
    }

    pub(crate) async fn apply(&self, mutation: DspMutation) -> PipelineResult<()> {
        let (acknowledgement, response) = oneshot::channel();
        self.sender
            .send(DspCommand {
                mutation,
                acknowledgement,
            })
            .await
            .map_err(|_| PipelineError::WorkerClosed)?;
        response.await.map_err(|_| PipelineError::WorkerClosed)?
    }
}

pub(crate) fn apply_command(
    processor: &mut Option<AudioProcessor>,
    spatial_position: &mut SpatialPosition,
    command: DspCommand,
    transition_frames: usize,
) {
    let result = match command.mutation {
        DspMutation::Gain(gain) => processor
            .as_mut()
            .ok_or(PipelineError::WorkerClosed)
            .map(|processor| processor.set_output_gain(gain, transition_frames)),
        DspMutation::Spatial(enabled) => processor
            .as_mut()
            .ok_or(PipelineError::WorkerClosed)
            .map(|processor| processor.set_wet_enabled(enabled, transition_frames)),
        DspMutation::Orbit(position) => processor
            .as_mut()
            .ok_or(PipelineError::WorkerClosed)
            .and_then(|processor| processor.set_orbit_position(position).map_err(Into::into))
            .map(|()| spatial_position.rebase(position)),
        DspMutation::Preset {
            renderer,
            enable_wet,
        } => processor
            .as_mut()
            .ok_or(PipelineError::WorkerClosed)
            .and_then(|processor| {
                processor
                    .install_prepared_renderer(renderer, transition_frames)
                    .map_err(Into::into)
                    .map(|()| processor.set_wet_enabled(enable_wet, transition_frames))
            }),
    };
    let _ = command.acknowledgement.send(result);
}
