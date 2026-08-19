use std::{path::Path, path::PathBuf, time::Duration};

use async_trait::async_trait;
use tokio::{
    io::AsyncReadExt,
    process::{Child, ChildStdout, Command},
    sync::OwnedSemaphorePermit,
    task::JoinHandle,
    time::Instant,
};

use super::{
    CommandSpec, ProcessPool,
    process::{drain_bounded, sanitize_environment},
};
use crate::ProcessError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodeExit {
    pub status_code: Option<i32>,
}

/// Fakeable streaming PCM decoder lifecycle.
#[async_trait]
pub trait PcmDecoder: Send {
    /// Reads raw little-endian `f32` stereo samples.
    async fn read_pcm(&mut self, output: &mut [u8]) -> Result<usize, ProcessError>;
    /// Explicitly terminates and reaps the decoder.
    async fn shutdown(&mut self) -> Result<(), ProcessError>;
}

#[async_trait]
pub trait DecoderSpawner: Send + Sync {
    type Decoder: PcmDecoder;

    /// Spawns a decoder for an already-managed local object.
    async fn spawn(&self, path: &Path) -> Result<Self::Decoder, ProcessError>;
}

/// Direct `ffmpeg` adapter producing 48 kHz stereo `f32le` on stdout.
#[derive(Clone, Debug)]
pub struct Ffmpeg {
    program: PathBuf,
    pool: ProcessPool,
    spawn_timeout: Duration,
    max_stderr_bytes: usize,
}

impl Ffmpeg {
    /// # Errors
    ///
    /// Returns [`ProcessError::InvalidConfig`] for zero limits.
    pub fn new(
        program: impl Into<PathBuf>,
        pool: ProcessPool,
        spawn_timeout: Duration,
        max_stderr_bytes: usize,
    ) -> Result<Self, ProcessError> {
        if spawn_timeout.is_zero() || max_stderr_bytes == 0 {
            return Err(ProcessError::InvalidConfig);
        }
        Ok(Self {
            program: program.into(),
            pool,
            spawn_timeout,
            max_stderr_bytes,
        })
    }

    /// Builds the exact non-shell decoder invocation.
    #[must_use]
    pub fn command_spec(&self, path: &Path) -> CommandSpec {
        CommandSpec::new(
            self.program.clone(),
            vec![
                "-v".into(),
                "error".into(),
                "-nostdin".into(),
                "-protocol_whitelist".into(),
                "file,pipe".into(),
                "-i".into(),
                path.as_os_str().to_os_string(),
                "-map".into(),
                "0:a:0".into(),
                "-vn".into(),
                "-sn".into(),
                "-dn".into(),
                "-f".into(),
                "f32le".into(),
                "-acodec".into(),
                "pcm_f32le".into(),
                "-ar".into(),
                "48000".into(),
                "-ac".into(),
                "2".into(),
                "pipe:1".into(),
            ],
        )
    }
}

#[async_trait]
impl DecoderSpawner for Ffmpeg {
    type Decoder = FfmpegDecoder;

    async fn spawn(&self, path: &Path) -> Result<Self::Decoder, ProcessError> {
        let deadline = Instant::now()
            .checked_add(self.spawn_timeout)
            .ok_or(ProcessError::InvalidConfig)?;
        let permit = self.pool.acquire_until(deadline).await?;
        let specification = self.command_spec(path);
        let mut command = Command::new(specification.program());
        sanitize_environment(&mut command);
        command
            .args(specification.arguments())
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        let mut child = command.spawn().map_err(ProcessError::Spawn)?;
        let stdout = child.stdout.take().ok_or(ProcessError::MissingStdout)?;
        let stderr = child.stderr.take().ok_or(ProcessError::MissingStdout)?;
        let stderr_task = tokio::spawn(drain_bounded(stderr, self.max_stderr_bytes));
        Ok(FfmpegDecoder {
            child: Some(child),
            stdout: Some(stdout),
            stderr_task: Some(stderr_task),
            permit: Some(permit),
        })
    }
}

/// Owned decoder child with explicit shutdown and kill-on-drop fallback.
pub struct FfmpegDecoder {
    child: Option<Child>,
    stdout: Option<ChildStdout>,
    stderr_task: Option<JoinHandle<Result<Vec<u8>, ProcessError>>>,
    permit: Option<OwnedSemaphorePermit>,
}

impl FfmpegDecoder {
    /// Waits for natural completion after stdout has been consumed to EOF.
    ///
    /// # Errors
    ///
    /// Returns an error for a failed process or pipe-drain operation.
    pub async fn wait_for_exit(&mut self) -> Result<DecodeExit, ProcessError> {
        self.stdout.take();
        let status = self
            .child
            .as_mut()
            .ok_or_else(lifecycle_already_finished)?
            .wait()
            .await
            .map_err(ProcessError::Lifecycle)?;
        self.child.take();
        finish_stderr(&mut self.stderr_task).await?;
        self.permit.take();
        if !status.success() {
            return Err(ProcessError::Exit {
                code: status.code(),
            });
        }
        Ok(DecodeExit {
            status_code: status.code(),
        })
    }

    async fn terminate(&mut self) -> Result<(), ProcessError> {
        self.stdout.take();
        if let Some(mut child) = self.child.take() {
            if child.try_wait().map_err(ProcessError::Lifecycle)?.is_none() {
                child.kill().await.map_err(ProcessError::Lifecycle)?;
            }
            let _ = child.wait().await.map_err(ProcessError::Lifecycle)?;
        }
        finish_stderr(&mut self.stderr_task).await?;
        self.permit.take();
        Ok(())
    }
}

#[async_trait]
impl PcmDecoder for FfmpegDecoder {
    async fn read_pcm(&mut self, output: &mut [u8]) -> Result<usize, ProcessError> {
        self.stdout
            .as_mut()
            .ok_or(ProcessError::MissingStdout)?
            .read(output)
            .await
            .map_err(ProcessError::Pipe)
    }

    async fn shutdown(&mut self) -> Result<(), ProcessError> {
        self.terminate().await
    }
}

impl Drop for FfmpegDecoder {
    fn drop(&mut self) {
        self.stdout.take();
        if let Some(child) = self.child.as_mut() {
            let _ = child.start_kill();
        }
        if let Some(task) = self.stderr_task.take() {
            task.abort();
        }
        self.permit.take();
    }
}

async fn finish_stderr(
    task: &mut Option<JoinHandle<Result<Vec<u8>, ProcessError>>>,
) -> Result<(), ProcessError> {
    let Some(task) = task.take() else {
        return Ok(());
    };
    task.await.map_err(|error| {
        ProcessError::Pipe(std::io::Error::other(format!(
            "ffmpeg stderr task failed: {error}"
        )))
    })??;
    Ok(())
}

fn lifecycle_already_finished() -> ProcessError {
    ProcessError::Lifecycle(std::io::Error::other("decoder has already finished"))
}
