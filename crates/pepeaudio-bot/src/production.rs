use std::{num::NonZeroU32, sync::Arc, time::Duration};

use crate::{
    BotConfig, BotError, PlayerRegistry,
    dashboard_command_executor::DashboardCommandExecutor,
    discord_status::DiscordStatusRuntime,
    guild_lifecycle::GuildLifecycleRuntime,
    production_build::assemble,
    production_janitor::MediaJanitorRuntime,
    runtime::{build_client, start_client},
    shutdown::ShutdownDeadline,
    web_authorizer::DiscordCommandAuthorizer,
};
use pepeaudio_runtime::{
    CommandWorkerConfig, CommandWorkerRuntime, DEFAULT_COMMAND_RESULT_RETENTION,
    GuildPresenceRuntime, SettingsPersistenceRuntime, SnapshotPublisherRuntime,
};
use pepeaudio_storage::{PostgresStorage, ValkeyStore};

pub(crate) async fn run(discord: BotConfig) -> Result<(), BotError> {
    let runtime = &discord.runtime;
    let services = assemble(&discord, runtime).await?;
    let dashboard_media = services.data.media.clone();
    let mut client =
        build_client(&discord, Arc::new(services.data), services.manager.clone()).await?;
    let mut discord_status = DiscordStatusRuntime::start(client.shard_manager.clone());
    let shard_total = NonZeroU32::new(discord.shards().total())
        .expect("BotConfig validates a non-zero shard total");
    let authorizer = Arc::new(DiscordCommandAuthorizer::new(
        client.cache.clone(),
        services.postgres.clone(),
        shard_total,
        discord.shards().range(),
    ));
    let executor = Arc::new(DashboardCommandExecutor::new(
        services.players.clone(),
        dashboard_media,
        authorizer.clone(),
    ));
    let mut command_worker = CommandWorkerRuntime::start(
        services.valkey,
        executor,
        authorizer,
        command_worker_config(&discord, &runtime.instance_id),
    )
    .await
    .map_err(|_| BotError::RuntimeDependency)?;
    let mut janitor =
        MediaJanitorRuntime::start(services.media_janitor.clone(), Duration::from_mins(15));
    let mut presence = services.presence;
    let mut guild_lifecycle = services.guild_lifecycle;
    let mut settings_persistence = services.settings_persistence;
    let supervision = supervise(
        &mut client,
        &discord,
        SupervisedRuntimes {
            commands: &mut command_worker,
            presence: &mut presence,
            guild_lifecycle: &mut guild_lifecycle,
            janitor: &mut janitor,
            settings: &mut settings_persistence,
            discord_status: &mut discord_status,
        },
    )
    .await;
    let cleanup_result = if let Some(result) = supervision
        .deadline
        .run(shutdown(ShutdownResources {
            command_worker,
            guild_lifecycle,
            presence,
            media_janitor: janitor,
            players: &services.players,
            snapshots: services.snapshots,
            settings_persistence,
            discord_status,
            postgres: &services.postgres,
        }))
        .await
    {
        result
    } else {
        // Dropping the cleanup future also drops every owned runtime. Their
        // Drop implementations signal or abort spawned tasks; no detached
        // command, lifecycle, presence, janitor, status, snapshot, or settings
        // worker survives the process-wide timeout.
        tracing::error!("Bot cleanup exceeded the process shutdown budget");
        Err(BotError::Shutdown)
    };
    supervision.result?;
    cleanup_result
}

struct Supervision {
    result: Result<(), BotError>,
    deadline: ShutdownDeadline,
}

enum StopReason {
    Signal,
    Gateway(Result<(), BotError>),
    Commands(pepeaudio_runtime::RuntimeError),
    Presence(pepeaudio_runtime::RuntimeError),
    GuildLifecycle(crate::guild_lifecycle::GuildLifecycleTaskError),
    MediaJanitor(BotError),
    Settings(pepeaudio_runtime::SettingsSupervisorError),
    DiscordStatus(BotError),
}

struct SupervisedRuntimes<'a> {
    commands: &'a mut CommandWorkerRuntime,
    presence: &'a mut GuildPresenceRuntime,
    guild_lifecycle: &'a mut GuildLifecycleRuntime,
    janitor: &'a mut MediaJanitorRuntime,
    settings: &'a mut SettingsPersistenceRuntime<PostgresStorage>,
    discord_status: &'a mut DiscordStatusRuntime,
}

async fn supervise(
    client: &mut serenity::Client,
    discord: &BotConfig,
    runtimes: SupervisedRuntimes<'_>,
) -> Supervision {
    let SupervisedRuntimes {
        commands,
        presence,
        guild_lifecycle,
        janitor,
        settings,
        discord_status,
    } = runtimes;
    let shard_manager = client.shard_manager.clone();
    let gateway = start_client(client, discord);
    tokio::pin!(gateway);

    let stop_reason = tokio::select! {
        biased;
        () = crate::shutdown::signal() => StopReason::Signal,
        result = &mut gateway => StopReason::Gateway(result),
        error = commands.wait_for_unexpected_exit() => StopReason::Commands(error),
        error = presence.wait_for_unexpected_exit() => StopReason::Presence(error),
        error = guild_lifecycle.wait_for_unexpected_exit() => StopReason::GuildLifecycle(error),
        error = janitor.wait_for_unexpected_exit() => StopReason::MediaJanitor(error),
        error = settings.wait_for_unexpected_exit() => StopReason::Settings(error),
        error = discord_status.wait_for_unexpected_exit() => StopReason::DiscordStatus(error),
    };
    let deadline = ShutdownDeadline::begin();
    let gateway_clean = if matches!(&stop_reason, StopReason::Gateway(_)) {
        true
    } else {
        match deadline
            .run(async {
                shard_manager.shutdown_all().await;
                gateway.as_mut().await
            })
            .await
        {
            Some(Ok(())) => true,
            Some(Err(error)) => {
                tracing::warn!(error = %error, "Discord gateway shutdown failed");
                false
            }
            None => {
                tracing::error!("Discord gateway shutdown exceeded the process shutdown budget");
                false
            }
        }
    };
    let result = match stop_reason {
        StopReason::Signal if gateway_clean => Ok(()),
        StopReason::Signal => Err(BotError::Shutdown),
        StopReason::Gateway(Ok(())) => Err(BotError::GatewayStopped),
        StopReason::Gateway(Err(error)) => Err(error),
        StopReason::Commands(error) => {
            tracing::error!(error = %error, "mandatory command worker stopped");
            Err(BotError::RuntimeTask(error))
        }
        StopReason::Presence(error) => {
            tracing::error!(error = %error, "mandatory guild presence task stopped");
            Err(BotError::RuntimeTask(error))
        }
        StopReason::GuildLifecycle(error) => {
            tracing::error!(error = %error, "mandatory guild lifecycle actor stopped");
            Err(BotError::GuildLifecycleTask)
        }
        StopReason::MediaJanitor(error) => {
            tracing::error!(error = %error, "mandatory media janitor task stopped");
            Err(error)
        }
        StopReason::Settings(error) => {
            tracing::error!(error = %error, "mandatory settings persistence worker stopped");
            Err(BotError::SettingsPersistence(error))
        }
        StopReason::DiscordStatus(error) => {
            tracing::error!(error = %error, "Discord status updater stopped");
            Err(error)
        }
    };
    Supervision { result, deadline }
}

fn command_worker_config(discord: &BotConfig, instance_id: &str) -> CommandWorkerConfig {
    CommandWorkerConfig {
        shards: discord.shards().range(),
        group: "pepeaudio-bot".into(),
        consumer: instance_id.into(),
        batch_size: 32,
        block: Duration::from_secs(1),
        claim_idle: Duration::from_secs(30),
        // Dashboard media commands may spend up to five minutes in the bounded
        // resolver. The lease must outlive that window so another worker cannot
        // resolve and enqueue the same command concurrently.
        lease_ttl: Duration::from_mins(6),
        completion_retention: DEFAULT_COMMAND_RESULT_RETENTION,
        retry_delay: Duration::from_secs(1),
    }
}

struct ShutdownResources<'a> {
    command_worker: CommandWorkerRuntime,
    guild_lifecycle: GuildLifecycleRuntime,
    presence: GuildPresenceRuntime,
    media_janitor: MediaJanitorRuntime,
    players: &'a PlayerRegistry,
    snapshots: SnapshotPublisherRuntime<ValkeyStore>,
    settings_persistence: SettingsPersistenceRuntime<PostgresStorage>,
    discord_status: DiscordStatusRuntime,
    postgres: &'a PostgresStorage,
}

async fn shutdown(resources: ShutdownResources<'_>) -> Result<(), BotError> {
    let ShutdownResources {
        command_worker,
        guild_lifecycle,
        presence,
        media_janitor,
        players,
        snapshots,
        settings_persistence,
        discord_status,
        postgres,
    } = resources;
    // Stop both command ingress paths before shutting down authoritative guild
    // players. Neither runtime depends on the other during coordinated stop.
    let (command_result, lifecycle_result) =
        tokio::join!(command_worker.shutdown(), guild_lifecycle.shutdown());
    let mut clean = command_result.is_ok() && lifecycle_result.is_ok();

    // Presence leases, expired media cleanup, and voice/player cleanup have no
    // ordering dependency and share the remaining process-wide deadline.
    let (presence_result, janitor_clean, player_result, status_clean) = tokio::join!(
        presence.shutdown(),
        media_janitor.shutdown(),
        players.shutdown_all(),
        discord_status.shutdown()
    );
    clean &= presence_result.is_ok() && janitor_clean && player_result.is_ok() && status_clean;

    // Players can publish final state while stopping, so drain their publishers
    // only after every player received a cleanup attempt.
    let (snapshot_result, settings_result) =
        tokio::join!(snapshots.shutdown(), settings_persistence.shutdown());
    clean &= snapshot_result.is_ok() && settings_result.is_ok();
    postgres.close().await;
    if clean {
        Ok(())
    } else {
        Err(BotError::Shutdown)
    }
}
