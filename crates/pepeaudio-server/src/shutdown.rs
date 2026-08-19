use std::{future::Future, time::Duration};

pub(crate) const HTTP_DRAIN_TIMEOUT: Duration = Duration::from_secs(15);
pub(crate) const BACKEND_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);
pub(crate) const POSTGRES_CLOSE_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum BoundedOutcome<T> {
    Completed(T),
    TimedOut,
}

pub(crate) async fn within<F>(future: F, timeout: Duration) -> BoundedOutcome<F::Output>
where
    F: Future,
{
    match tokio::time::timeout(timeout, future).await {
        Ok(output) => BoundedOutcome::Completed(output),
        Err(_) => BoundedOutcome::TimedOut,
    }
}

pub(crate) async fn finish_dependencies<R, P>(
    runtime: R,
    postgres: P,
) -> (BoundedOutcome<R::Output>, BoundedOutcome<P::Output>)
where
    R: Future,
    P: Future,
{
    let runtime = within(runtime, BACKEND_SHUTDOWN_TIMEOUT).await;
    let postgres = within(postgres, POSTGRES_CLOSE_TIMEOUT).await;
    (runtime, postgres)
}

pub(crate) async fn signal() {
    let ctrl_c = async {
        let _result = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                let _signal = signal.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {}
        () = terminate => {}
    }
}

#[cfg(test)]
mod tests {
    use std::{
        future,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        time::Duration,
    };

    use tokio::time::Instant;

    use super::{
        BACKEND_SHUTDOWN_TIMEOUT, BoundedOutcome, HTTP_DRAIN_TIMEOUT, POSTGRES_CLOSE_TIMEOUT,
        finish_dependencies, within,
    };

    #[tokio::test]
    async fn completed_work_preserves_its_output() {
        assert_eq!(
            within(async { 42 }, Duration::from_secs(5)).await,
            BoundedOutcome::Completed(42)
        );
    }

    #[tokio::test(start_paused = true)]
    async fn pending_work_is_released_at_the_configured_deadline() {
        let started = Instant::now();
        let outcome = within(future::pending::<()>(), Duration::from_secs(5)).await;

        assert_eq!(outcome, BoundedOutcome::TimedOut);
        assert_eq!(Instant::now() - started, Duration::from_secs(5));
    }

    #[tokio::test(start_paused = true)]
    async fn postgres_close_is_polled_after_a_stuck_runtime_times_out() {
        let postgres_polled = Arc::new(AtomicBool::new(false));
        let observed = postgres_polled.clone();

        let (runtime, postgres) = finish_dependencies(future::pending::<()>(), async move {
            observed.store(true, Ordering::SeqCst);
        })
        .await;

        assert_eq!(runtime, BoundedOutcome::TimedOut);
        assert_eq!(postgres, BoundedOutcome::Completed(()));
        assert!(postgres_polled.load(Ordering::SeqCst));
    }

    #[test]
    fn shutdown_budget_fits_inside_the_compose_stop_window() {
        let total = HTTP_DRAIN_TIMEOUT + BACKEND_SHUTDOWN_TIMEOUT + POSTGRES_CLOSE_TIMEOUT;
        assert!(total < Duration::from_secs(30));
    }
}
