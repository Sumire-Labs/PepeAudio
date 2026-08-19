use std::{path::PathBuf, sync::Arc, time::Duration};

use async_trait::async_trait;
use pepeaudio_media::CommandSpec;
use tokio::{process::Command, sync::Semaphore, time::timeout};

use super::{
    DecodedPcm, DecoderFactory, DecoderProcessSlot, DecoderReplacementPermit, SpawnedDecoder,
    process::FfmpegDecodedPcm, spec::ffmpeg_spec,
};
use crate::{PipelineError, PipelineResult, ResolvedSource};

/// Bounded `FFmpeg` process factory producing 48 kHz stereo `f32le` PCM.
#[derive(Clone, Debug)]
pub struct FfmpegDecoderFactory {
    program: PathBuf,
    permits: Arc<Semaphore>,
    replacement_permits: Arc<Semaphore>,
    pool_owner: Arc<()>,
    permit_timeout: Duration,
    spawn_timeout: Duration,
    max_stderr_bytes: usize,
    maximum_duration: Duration,
}

impl FfmpegDecoderFactory {
    /// # Errors
    ///
    /// Returns [`PipelineError::InvalidConfig`] for any zero limit.
    pub fn new(
        program: impl Into<PathBuf>,
        maximum_processes: usize,
        permit_timeout: Duration,
        spawn_timeout: Duration,
        max_stderr_bytes: usize,
        maximum_duration: Duration,
    ) -> PipelineResult<Self> {
        let program = program.into();
        if program.as_os_str().is_empty()
            || maximum_processes == 0
            || permit_timeout.is_zero()
            || spawn_timeout.is_zero()
            || max_stderr_bytes == 0
            || maximum_duration.is_zero()
        {
            return Err(PipelineError::InvalidConfig);
        }
        Ok(Self {
            program,
            permits: Arc::new(Semaphore::new(maximum_processes)),
            replacement_permits: Arc::new(Semaphore::new(1)),
            pool_owner: Arc::new(()),
            permit_timeout,
            spawn_timeout,
            max_stderr_bytes,
            maximum_duration,
        })
    }

    #[must_use]
    pub fn command_spec(&self, source: &ResolvedSource, start_offset: Duration) -> CommandSpec {
        ffmpeg_spec(
            &self.program,
            source.path(),
            start_offset,
            self.maximum_duration,
        )
    }

    async fn spawn_child(
        &self,
        specification: &CommandSpec,
    ) -> PipelineResult<tokio::process::Child> {
        let mut command = Command::new(specification.program());
        sanitize_environment(&mut command);
        command
            .args(specification.arguments())
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);

        // Yielding makes the startup deadline observable before the synchronous
        // OS spawn call. Process creation itself cannot be pre-empted portably.
        timeout(self.spawn_timeout, async move {
            tokio::task::yield_now().await;
            command.spawn().map_err(PipelineError::DecoderSpawn)
        })
        .await
        .map_err(|_| PipelineError::Timeout)?
    }

    pub(super) async fn acquire_process_slot(&self) -> PipelineResult<DecoderProcessSlot> {
        let permit = timeout(
            self.permit_timeout,
            Arc::clone(&self.permits).acquire_owned(),
        )
        .await
        .map_err(|_| PipelineError::Timeout)?
        .map_err(|_| PipelineError::InvalidConfig)?;
        Ok(DecoderProcessSlot::tracked(
            Arc::clone(&self.pool_owner),
            permit,
        ))
    }

    pub(super) async fn acquire_replacement_permit(
        &self,
        active_slot: &DecoderProcessSlot,
    ) -> PipelineResult<DecoderReplacementPermit> {
        if !active_slot.belongs_to(&self.pool_owner) {
            return Err(PipelineError::InvalidConfig);
        }
        let permit = timeout(
            self.permit_timeout,
            Arc::clone(&self.replacement_permits).acquire_owned(),
        )
        .await
        .map_err(|_| PipelineError::Timeout)?
        .map_err(|_| PipelineError::InvalidConfig)?;
        Ok(DecoderReplacementPermit::tracked(permit))
    }

    async fn spawn_decoder(
        &self,
        source: &ResolvedSource,
        start_offset: Duration,
    ) -> PipelineResult<Box<dyn DecodedPcm>> {
        let specification = self.command_spec(source, start_offset);
        let child = self.spawn_child(&specification).await?;
        let decoder = FfmpegDecodedPcm::new(child, self.max_stderr_bytes).await?;
        Ok(Box::new(decoder))
    }
}

fn sanitize_environment(command: &mut Command) {
    command.env_clear();
    if let Some(path) = std::env::var_os("PATH") {
        command.env("PATH", path);
    }
    #[cfg(windows)]
    for name in ["SystemRoot", "WINDIR"] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    #[cfg(unix)]
    command.env("LANG", "C").env("LC_ALL", "C");
}

#[async_trait]
impl DecoderFactory for FfmpegDecoderFactory {
    async fn spawn(
        &self,
        source: &ResolvedSource,
        start_offset: Duration,
    ) -> PipelineResult<SpawnedDecoder> {
        let slot = self.acquire_process_slot().await?;
        let decoder = self.spawn_decoder(source, start_offset).await?;
        Ok(SpawnedDecoder::stable(decoder, slot))
    }

    async fn spawn_replacement(
        &self,
        source: &ResolvedSource,
        start_offset: Duration,
        active_slot: &DecoderProcessSlot,
    ) -> PipelineResult<SpawnedDecoder> {
        let replacement_permit = self.acquire_replacement_permit(active_slot).await?;
        let decoder = self.spawn_decoder(source, start_offset).await?;
        Ok(SpawnedDecoder::replacement(
            decoder,
            active_slot.clone(),
            replacement_permit,
        ))
    }
}
