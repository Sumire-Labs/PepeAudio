use std::{future::Future, time::Duration};

use tokio::time::Instant;

/// Docker gives the Bot 45 seconds before sending `SIGKILL`. Reserve seven
/// seconds for scheduler delay, log flushing, and runtime teardown after this
/// future returns to `main`.
pub(crate) const PROCESS_SHUTDOWN_BUDGET: Duration = Duration::from_secs(38);
#[cfg(test)]
pub(crate) const COMPOSE_STOP_GRACE_PERIOD: Duration = Duration::from_secs(45);

/// One absolute deadline shared by every shutdown phase.
#[derive(Clone, Copy)]
pub(crate) struct ShutdownDeadline {
    expires_at: Instant,
}

impl ShutdownDeadline {
    #[must_use]
    pub(crate) fn begin() -> Self {
        Self {
            expires_at: Instant::now() + PROCESS_SHUTDOWN_BUDGET,
        }
    }

    /// Polls work only for the time remaining in the process-wide budget.
    pub(crate) async fn run<F>(self, work: F) -> Option<F::Output>
    where
        F: Future,
    {
        tokio::time::timeout_at(self.expires_at, work).await.ok()
    }
}

pub(crate) async fn signal() {
    #[cfg(unix)]
    {
        let Ok(mut terminate) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        else {
            let _ = tokio::signal::ctrl_c().await;
            return;
        };
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = terminate.recv() => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

#[cfg(test)]
mod tests {
    use std::{future, time::Duration};

    use tokio::time::Instant;

    use super::{COMPOSE_STOP_GRACE_PERIOD, PROCESS_SHUTDOWN_BUDGET, ShutdownDeadline};

    #[test]
    fn process_budget_leaves_time_before_docker_kills_the_container() {
        let compose = include_str!("../../../compose.discord.yaml");
        assert!(compose.contains("stop_grace_period: 45s"));
        assert!(PROCESS_SHUTDOWN_BUDGET < COMPOSE_STOP_GRACE_PERIOD);
        assert_eq!(
            COMPOSE_STOP_GRACE_PERIOD
                .checked_sub(PROCESS_SHUTDOWN_BUDGET)
                .expect("shutdown budget is below the Compose grace period"),
            Duration::from_secs(7)
        );
    }

    #[tokio::test(start_paused = true)]
    async fn dependency_hang_is_cancelled_at_the_process_deadline() {
        let started = Instant::now();
        let deadline = ShutdownDeadline::begin();

        assert!(deadline.run(future::pending::<()>()).await.is_none());
        assert_eq!(Instant::now() - started, PROCESS_SHUTDOWN_BUDGET);
    }

    #[tokio::test(start_paused = true)]
    async fn later_phases_use_remaining_time_instead_of_resetting_the_budget() {
        let started = Instant::now();
        let deadline = ShutdownDeadline::begin();

        assert_eq!(
            deadline
                .run(tokio::time::sleep(Duration::from_secs(30)))
                .await,
            Some(())
        );
        assert!(deadline.run(future::pending::<()>()).await.is_none());
        assert_eq!(Instant::now() - started, PROCESS_SHUTDOWN_BUDGET);
    }
}
