use std::time::Duration;

use tokio::{process::Command, task::JoinHandle, time::timeout};

#[cfg(unix)]
use nix::{
    sys::signal::{Signal, killpg},
    unistd::Pid,
};

use crate::ProcessError;

const TERMINATION_TIMEOUT: Duration = Duration::from_secs(2);

#[cfg(unix)]
pub(super) type ProcessGroup = Option<i32>;
#[cfg(not(unix))]
pub(super) type ProcessGroup = ();

#[cfg(unix)]
pub(super) struct ProcessGroupGuard {
    process_group: ProcessGroup,
    armed: bool,
}

#[cfg(unix)]
impl ProcessGroupGuard {
    pub(super) const fn new(process_group: ProcessGroup) -> Self {
        Self {
            process_group,
            armed: true,
        }
    }

    pub(super) const fn disarm(&mut self) {
        self.armed = false;
    }
}

#[cfg(unix)]
impl Drop for ProcessGroupGuard {
    fn drop(&mut self) {
        if self.armed {
            terminate_process_group(self.process_group);
        }
    }
}

#[cfg(not(unix))]
pub(super) struct ProcessGroupGuard {
    armed: bool,
}

#[cfg(not(unix))]
impl ProcessGroupGuard {
    pub(super) const fn new(_process_group: ProcessGroup) -> Self {
        Self { armed: true }
    }

    pub(super) const fn disarm(&mut self) {
        self.armed = false;
    }
}

pub(super) async fn terminate(child: &mut tokio::process::Child, process_group: ProcessGroup) {
    terminate_process_group(process_group);
    let _ = child.start_kill();
    let _ = timeout(TERMINATION_TIMEOUT, child.wait()).await;
}

pub(super) async fn join_after_termination(mut task: JoinHandle<Result<Vec<u8>, ProcessError>>) {
    if timeout(TERMINATION_TIMEOUT, &mut task).await.is_err() {
        task.abort();
        let _ = task.await;
    }
}

#[cfg(unix)]
pub(super) fn configure_process_group(command: &mut Command) {
    command.process_group(0);
}

#[cfg(not(unix))]
pub(super) fn configure_process_group(_command: &mut Command) {}

#[cfg(unix)]
pub(super) fn process_group(child: &tokio::process::Child) -> ProcessGroup {
    child
        .id()
        .and_then(|identifier| i32::try_from(identifier).ok())
}

#[cfg(not(unix))]
pub(super) fn process_group(_child: &tokio::process::Child) -> ProcessGroup {}

#[cfg(unix)]
pub(super) fn terminate_process_group(process_group: ProcessGroup) {
    if let Some(identifier) = process_group {
        let _ = killpg(Pid::from_raw(identifier), Signal::SIGKILL);
    }
}

#[cfg(not(unix))]
pub(super) fn terminate_process_group(_process_group: ProcessGroup) {}
