use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum StartupError {
    #[error("required environment variable {0} is missing")]
    MissingEnvironment(&'static str),
    #[error("environment variable {0} is invalid")]
    InvalidEnvironment(&'static str),
    #[error(transparent)]
    ApiConfig(#[from] pepeaudio_api::ConfigError),
    #[error(transparent)]
    AppConfig(#[from] pepeaudio_config::ConfigError),
    #[error(transparent)]
    AuthConfig(#[from] pepeaudio_auth::AuthConfigError),
    #[error("authentication dependency unavailable")]
    AuthDependency,
    #[error("runtime dependency unavailable")]
    RuntimeDependency,
    #[error("{0} did not stop before its shutdown deadline")]
    ShutdownTimeout(&'static str),
    #[error("a mandatory API runtime task stopped unexpectedly")]
    RuntimeTask(#[source] pepeaudio_runtime::RuntimeError),
    #[error("storage dependency unavailable")]
    StorageDependency,
    #[error(transparent)]
    DevAuth(#[from] pepeaudio_api::DevAuthConfigError),
    #[error(transparent)]
    Authorization(#[from] pepeaudio_api::AuthorizationError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
