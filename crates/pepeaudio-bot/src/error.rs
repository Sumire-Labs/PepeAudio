use thiserror::Error;

#[derive(Debug, Error)]
pub enum BotError {
    #[error(transparent)]
    Configuration(#[from] crate::ConfigError),
    #[error("a production storage dependency is unavailable")]
    StorageDependency,
    #[error("the HRIR catalog is unavailable or empty")]
    HrirCatalog,
    #[error("the production media adapter could not start")]
    MediaAdapter,
    #[error(
        "site-specific media extractors are enabled but no audited extractor adapter is installed"
    )]
    SiteExtractorsUnavailable,
    #[error("the production player factory could not start")]
    PlayerFactory,
    #[error("the shard command or presence runtime could not start")]
    RuntimeDependency,
    #[error("a mandatory Bot runtime task stopped unexpectedly")]
    RuntimeTask(#[source] pepeaudio_runtime::RuntimeError),
    #[error("the mandatory guild lifecycle actor stopped unexpectedly")]
    GuildLifecycleTask,
    #[error("a mandatory durable-settings task stopped unexpectedly")]
    SettingsPersistence(#[source] pepeaudio_runtime::SettingsSupervisorError),
    #[error("a production runtime task could not shut down cleanly")]
    Shutdown,
    #[error("failed to build the Discord client: {0}")]
    ClientBuild(#[source] Box<serenity::Error>),
    #[error("Discord gateway runtime stopped: {0}")]
    Gateway(#[source] Box<serenity::Error>),
    #[error("Discord gateway runtime stopped without a shutdown signal")]
    GatewayStopped,
    #[error("managed media cleanup stopped without a shutdown signal")]
    MediaJanitorStopped,
    #[error("managed media cleanup failed")]
    MediaJanitor(#[source] pepeaudio_media::JanitorError),
    #[error("managed media cleanup task failed")]
    MediaJanitorTask(#[source] tokio::task::JoinError),
    #[error("Discord status updater stopped without a shutdown signal")]
    DiscordStatusStopped,
    #[error("Discord status updater task failed")]
    DiscordStatusTask(#[source] tokio::task::JoinError),
}
