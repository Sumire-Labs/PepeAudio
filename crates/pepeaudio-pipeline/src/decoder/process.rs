use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::{Child, ChildStderr, ChildStdout},
    task::JoinHandle,
};

use super::DecodedPcm;
use crate::{PipelineError, PipelineResult};

/// Owned `FFmpeg` child with bounded diagnostics and kill-on-drop fallback.
pub(super) struct FfmpegDecodedPcm {
    child: Option<Child>,
    stdout: Option<ChildStdout>,
    stderr_task: Option<JoinHandle<std::io::Result<bool>>>,
}

impl FfmpegDecodedPcm {
    pub(super) async fn new(mut child: Child, max_stderr_bytes: usize) -> PipelineResult<Self> {
        let Some(stdout) = child.stdout.take() else {
            return cleanup_failed_spawn(child, PipelineError::DecoderPipe(missing_pipe())).await;
        };
        let Some(stderr) = child.stderr.take() else {
            return cleanup_failed_spawn(child, PipelineError::DecoderPipe(missing_pipe())).await;
        };
        let stderr_task = tokio::spawn(drain_stderr(stderr, max_stderr_bytes));
        Ok(Self {
            child: Some(child),
            stdout: Some(stdout),
            stderr_task: Some(stderr_task),
        })
    }

    async fn finish_stderr(&mut self) -> PipelineResult<bool> {
        let Some(task) = self.stderr_task.as_mut() else {
            return Ok(false);
        };
        let result = task
            .await
            .map_err(|error| PipelineError::DecoderPipe(std::io::Error::other(error)))?
            .map_err(PipelineError::DecoderPipe);
        self.stderr_task.take();
        result
    }

    async fn reap(&mut self, terminate: bool) -> PipelineResult<std::process::ExitStatus> {
        self.stdout.take();
        let child = self.child.as_mut().ok_or_else(finished_error)?;
        if terminate
            && child
                .try_wait()
                .map_err(PipelineError::DecoderLifecycle)?
                .is_none()
        {
            child
                .kill()
                .await
                .map_err(PipelineError::DecoderLifecycle)?;
        }
        let status = child
            .wait()
            .await
            .map_err(PipelineError::DecoderLifecycle)?;
        self.child.take();
        Ok(status)
    }
}

#[async_trait::async_trait]
impl DecodedPcm for FfmpegDecodedPcm {
    async fn read_pcm(&mut self, output: &mut [u8]) -> PipelineResult<usize> {
        if output.is_empty() {
            return Err(PipelineError::InvalidConfig);
        }
        let stdout = self.stdout.as_mut().ok_or_else(missing_stdout_error)?;
        stdout
            .read(output)
            .await
            .map_err(PipelineError::DecoderPipe)
    }

    async fn finish(&mut self) -> PipelineResult<()> {
        let status = self.reap(false).await?;
        let diagnostics_too_large = self.finish_stderr().await?;
        if !status.success() {
            return Err(PipelineError::DecoderExit {
                code: status.code(),
            });
        }
        if diagnostics_too_large {
            return Err(PipelineError::DecoderDiagnosticsTooLarge);
        }
        Ok(())
    }

    async fn shutdown(&mut self) -> PipelineResult<()> {
        if self.child.is_none() {
            return Ok(());
        }
        let _ = self.reap(true).await?;
        let _ = self.finish_stderr().await?;
        Ok(())
    }
}

impl Drop for FfmpegDecodedPcm {
    fn drop(&mut self) {
        self.stdout.take();
        if let Some(child) = self.child.as_mut() {
            let _ = child.start_kill();
        }
        if let Some(task) = self.stderr_task.take() {
            task.abort();
        }
    }
}

async fn drain_stderr(mut stderr: ChildStderr, maximum: usize) -> std::io::Result<bool> {
    drain_bounded_discard(&mut stderr, maximum).await
}

pub(super) async fn drain_bounded_discard<R: AsyncRead + Unpin>(
    reader: &mut R,
    maximum: usize,
) -> std::io::Result<bool> {
    let mut total = 0_u64;
    let mut buffer = [0_u8; 8 * 1024];
    loop {
        let read = reader.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        total = total.saturating_add(u64::try_from(read).unwrap_or(u64::MAX));
    }
    Ok(total > u64::try_from(maximum).unwrap_or(u64::MAX))
}

async fn cleanup_failed_spawn<T>(mut child: Child, error: PipelineError) -> PipelineResult<T> {
    let _ = child.kill().await;
    let _ = child.wait().await;
    Err(error)
}

fn missing_pipe() -> std::io::Error {
    std::io::Error::other("FFmpeg did not expose a configured pipe")
}

fn missing_stdout_error() -> PipelineError {
    PipelineError::DecoderPipe(std::io::Error::other("decoder stdout is unavailable"))
}

fn finished_error() -> PipelineError {
    PipelineError::DecoderLifecycle(std::io::Error::other("decoder is already finished"))
}
