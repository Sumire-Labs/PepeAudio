use std::{fmt, sync::Arc, time::Duration};

use async_trait::async_trait;
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
    sync::{OwnedSemaphorePermit, Semaphore},
    task::JoinHandle,
    time::{Instant, timeout_at},
};

use super::CommandSpec;
use crate::ProcessError;

mod termination;

use termination::{
    ProcessGroupGuard, configure_process_group, join_after_termination, process_group, terminate,
    terminate_process_group,
};

/// Shared bound for concurrently running external media tools.
#[derive(Clone, Debug)]
pub struct ProcessPool {
    permits: Arc<Semaphore>,
}

impl ProcessPool {
    /// # Errors
    ///
    /// Returns [`ProcessError::InvalidConfig`] when `maximum` is zero.
    pub fn new(maximum: usize) -> Result<Self, ProcessError> {
        if maximum == 0 {
            return Err(ProcessError::InvalidConfig);
        }
        Ok(Self {
            permits: Arc::new(Semaphore::new(maximum)),
        })
    }

    pub(crate) async fn acquire_until(
        &self,
        deadline: Instant,
    ) -> Result<OwnedSemaphorePermit, ProcessError> {
        timeout_at(deadline, Arc::clone(&self.permits).acquire_owned())
            .await
            .map_err(|_| ProcessError::Timeout)?
            .map_err(|_| ProcessError::InvalidConfig)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OutputLimits {
    /// Total time including waiting for a process permit.
    pub timeout: Duration,
    pub max_stdout_bytes: usize,
    pub max_stderr_bytes: usize,
}

impl OutputLimits {
    pub(crate) const fn is_valid(self) -> bool {
        !self.timeout.is_zero() && self.max_stdout_bytes > 0 && self.max_stderr_bytes > 0
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct ProcessOutput {
    pub status_code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

impl fmt::Debug for ProcessOutput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProcessOutput")
            .field("status_code", &self.status_code)
            .field("stdout_bytes", &self.stdout.len())
            .field("stderr_bytes", &self.stderr.len())
            .finish()
    }
}

/// Fakeable short-lived process boundary, used by `ffprobe`.
#[async_trait]
pub trait ProcessRunner: Send + Sync {
    /// Runs a direct command and returns bounded output.
    async fn run(
        &self,
        specification: &CommandSpec,
        limits: OutputLimits,
    ) -> Result<ProcessOutput, ProcessError>;
}

#[derive(Clone, Debug)]
pub struct RealProcessRunner {
    pool: ProcessPool,
}

impl RealProcessRunner {
    #[must_use]
    pub const fn new(pool: ProcessPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ProcessRunner for RealProcessRunner {
    async fn run(
        &self,
        specification: &CommandSpec,
        limits: OutputLimits,
    ) -> Result<ProcessOutput, ProcessError> {
        if !limits.is_valid() {
            return Err(ProcessError::InvalidConfig);
        }
        let deadline = Instant::now()
            .checked_add(limits.timeout)
            .ok_or(ProcessError::InvalidConfig)?;
        let _permit = self.pool.acquire_until(deadline).await?;
        let mut command = Command::new(specification.program());
        configure_environment(&mut command, specification);
        command
            .args(specification.arguments())
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        configure_process_group(&mut command);
        let mut child = command.spawn().map_err(ProcessError::Spawn)?;
        let process_group = process_group(&child);
        let mut process_group_guard = ProcessGroupGuard::new(process_group);
        let stdout = child.stdout.take().ok_or(ProcessError::MissingStdout)?;
        let stderr = child.stderr.take().ok_or(ProcessError::MissingStdout)?;
        let stdout_task = tokio::spawn(drain_bounded(stdout, limits.max_stdout_bytes));
        let stderr_task = tokio::spawn(drain_bounded(stderr, limits.max_stderr_bytes));

        let Ok(wait_result) = timeout_at(deadline, child.wait()).await else {
            terminate(&mut child, process_group).await;
            process_group_guard.disarm();
            join_after_termination(stdout_task).await;
            join_after_termination(stderr_task).await;
            return Err(ProcessError::Timeout);
        };
        if wait_result.is_err() {
            terminate(&mut child, process_group).await;
            process_group_guard.disarm();
            join_after_termination(stdout_task).await;
            join_after_termination(stderr_task).await;
            return Err(ProcessError::Lifecycle(
                wait_result.expect_err("wait result was checked as an error"),
            ));
        }
        let status = wait_result.expect("wait result was checked as successful");
        let stdout = match join_reader_until(stdout_task, deadline).await {
            Ok(stdout) => stdout,
            Err(error) => {
                terminate_process_group(process_group);
                process_group_guard.disarm();
                join_after_termination(stderr_task).await;
                return Err(error);
            }
        };
        let stderr = match join_reader_until(stderr_task, deadline).await {
            Ok(stderr) => stderr,
            Err(error) => {
                terminate_process_group(process_group);
                process_group_guard.disarm();
                return Err(error);
            }
        };
        terminate_process_group(process_group);
        process_group_guard.disarm();
        if !status.success() {
            if specification.should_classify_unavailable_media()
                && reports_unavailable_media(&stderr)
            {
                return Err(ProcessError::MediaUnavailable);
            }
            return Err(ProcessError::Exit {
                code: status.code(),
            });
        }
        Ok(ProcessOutput {
            status_code: status.code(),
            stdout,
            stderr,
        })
    }
}

pub(super) fn reports_unavailable_media(stderr: &[u8]) -> bool {
    let message = String::from_utf8_lossy(stderr).to_ascii_lowercase();
    [
        "private video",
        "video unavailable",
        "this video is unavailable",
        "this video has been removed",
        "this video is not available",
        "not available in your country",
        "not made this video available in your country",
        "members-only content",
        "join this channel to get access",
        "sign in to confirm your age",
        "this track was not found",
        "this track is not available",
        "this track is no longer available",
    ]
    .iter()
    .any(|phrase| message.contains(phrase))
}

pub(super) fn configure_environment(command: &mut Command, specification: &CommandSpec) {
    sanitize_environment(command);
    if let Some(directory) = specification.deno_directory() {
        command.env("DENO_DIR", directory);
    }
}

pub(super) fn sanitize_environment(command: &mut Command) {
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

pub(crate) async fn drain_bounded<R>(mut reader: R, maximum: usize) -> Result<Vec<u8>, ProcessError>
where
    R: AsyncRead + Unpin,
{
    let mut retained = Vec::with_capacity(maximum.min(8_192));
    let mut buffer = [0_u8; 8_192];
    let mut exceeded = false;
    loop {
        let read = reader.read(&mut buffer).await.map_err(ProcessError::Pipe)?;
        if read == 0 {
            break;
        }
        let remaining = maximum.saturating_sub(retained.len());
        let retain = remaining.min(read);
        retained.extend_from_slice(&buffer[..retain]);
        exceeded |= retain < read;
    }
    if exceeded {
        Err(ProcessError::OutputTooLarge)
    } else {
        Ok(retained)
    }
}

async fn join_reader_until(
    mut task: JoinHandle<Result<Vec<u8>, ProcessError>>,
    deadline: Instant,
) -> Result<Vec<u8>, ProcessError> {
    let Ok(result) = timeout_at(deadline, &mut task).await else {
        task.abort();
        let _ = task.await;
        return Err(ProcessError::Timeout);
    };
    result.map_err(|error| {
        ProcessError::Pipe(std::io::Error::other(format!(
            "media pipe task failed: {error}"
        )))
    })?
}
