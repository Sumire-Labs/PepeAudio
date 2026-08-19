use std::{future::Future, future::IntoFuture as _, pin::Pin};

use axum::Router;
use pepeaudio_api::ApiShutdown;
use pepeaudio_runtime::{ApiBackendRuntime, RuntimeError};
use pepeaudio_storage::PostgresStorage;

use crate::{error::StartupError, shutdown};

enum StopReason {
    Http(std::io::Result<()>),
    Signal,
    Runtime(RuntimeError),
}

pub(crate) async fn serve(
    listener: tokio::net::TcpListener,
    app: Router,
    api_shutdown: ApiShutdown,
    mut backend_runtime: ApiBackendRuntime,
    postgres: PostgresStorage,
) -> Result<(), StartupError> {
    let (http_shutdown, http_shutdown_receiver) = tokio::sync::oneshot::channel();
    let graceful_shutdown = async move {
        let _signal = http_shutdown_receiver.await;
    };
    let serve = axum::serve(listener, app)
        .with_graceful_shutdown(graceful_shutdown)
        .into_future();
    tokio::pin!(serve);

    let stop_reason = tokio::select! {
        result = &mut serve => StopReason::Http(result),
        () = shutdown::signal() => StopReason::Signal,
        error = backend_runtime.wait_for_unexpected_exit() => StopReason::Runtime(error),
    };
    let (serve_result, runtime_failure) = match stop_reason {
        StopReason::Http(result) => (result.map_err(StartupError::from), None),
        StopReason::Signal => {
            api_shutdown.trigger();
            let _signal = http_shutdown.send(());
            (drain_http(serve.as_mut()).await, None)
        }
        StopReason::Runtime(error) => {
            api_shutdown.trigger();
            let _signal = http_shutdown.send(());
            (drain_http(serve.as_mut()).await, Some(error))
        }
    };

    let (runtime_outcome, postgres_outcome) =
        shutdown::finish_dependencies(backend_runtime.shutdown(), postgres.close()).await;
    let runtime_result = match runtime_outcome {
        shutdown::BoundedOutcome::Completed(result) => {
            result.map_err(|_| StartupError::RuntimeDependency)
        }
        shutdown::BoundedOutcome::TimedOut => {
            Err(StartupError::ShutdownTimeout("API backend runtime"))
        }
    };
    let postgres_result = match postgres_outcome {
        shutdown::BoundedOutcome::Completed(()) => Ok(()),
        shutdown::BoundedOutcome::TimedOut => Err(StartupError::ShutdownTimeout("PostgreSQL pool")),
    };

    if let Some(error) = runtime_failure {
        return Err(StartupError::RuntimeTask(error));
    }
    serve_result?;
    runtime_result?;
    postgres_result
}

async fn drain_http<F>(serve: Pin<&mut F>) -> Result<(), StartupError>
where
    F: Future<Output = std::io::Result<()>>,
{
    match shutdown::within(serve, shutdown::HTTP_DRAIN_TIMEOUT).await {
        shutdown::BoundedOutcome::Completed(result) => result.map_err(StartupError::from),
        shutdown::BoundedOutcome::TimedOut => {
            eprintln!("pepeaudio-api HTTP drain timed out; closing remaining connections");
            Ok(())
        }
    }
}
