use std::{env, future::IntoFuture as _, net::SocketAddr, sync::Arc};

use pepeaudio_api::{
    ApiConfig, ApiShutdown, AppState, DEFAULT_COMMAND_TTL, DEV_USER_HEADER, DevHeaderAuthenticator,
    HrirPresetSummary, HrirSourceMetadata, SystemClock, build_router,
    dev::{AllowListAuthorizer, InMemoryPlayerBackend, StaticHrirPresetCatalog},
};
use pepeaudio_core::{
    GuildId, HrirPresetId, PlayerSnapshot, PlayerState, RepeatMode, StateRevision, UnixTimeMillis,
    UserId, Volume,
};

use crate::{error::StartupError, shutdown};

pub(crate) async fn run() -> Result<(), StartupError> {
    let bind = env_or("PEPEAUDIO_API_BIND", "0.0.0.0:8080")
        .parse::<SocketAddr>()
        .map_err(|_| StartupError::InvalidEnvironment("PEPEAUDIO_API_BIND"))?;
    let public_origin = env_or("PEPEAUDIO_PUBLIC_ORIGIN", "http://localhost:5173");
    let user_id = required("PEPEAUDIO_DEV_USER_ID")?
        .parse::<UserId>()
        .map_err(|_| StartupError::InvalidEnvironment("PEPEAUDIO_DEV_USER_ID"))?;
    let guild_id = required("PEPEAUDIO_DEV_GUILD_ID")?
        .parse::<GuildId>()
        .map_err(|_| StartupError::InvalidEnvironment("PEPEAUDIO_DEV_GUILD_ID"))?;
    let csrf_token = required("PEPEAUDIO_DEV_CSRF_TOKEN")?;

    eprintln!(
        "WARNING: development authentication is active; {DEV_USER_HEADER} must not be exposed publicly"
    );
    let backend = Arc::new(InMemoryPlayerBackend::new([empty_snapshot(guild_id)], 64));
    let authorizer = Arc::new(AllowListAuthorizer::new());
    authorizer.grant(user_id, guild_id)?;
    let api_shutdown = ApiShutdown::new();
    let state = AppState::new(
        ApiConfig::new(&public_origin, DEFAULT_COMMAND_TTL)?,
        Arc::new(DevHeaderAuthenticator::new(user_id, csrf_token)?),
        authorizer,
        backend.clone(),
        Arc::new(development_catalog()),
        backend.clone(),
        backend.clone(),
        backend.clone(),
        backend,
        Arc::new(SystemClock),
    )
    .with_shutdown(api_shutdown.clone());
    let listener = tokio::net::TcpListener::bind(bind).await?;
    eprintln!("pepeaudio-api development server listening on {bind}");
    let (http_shutdown, http_shutdown_receiver) = tokio::sync::oneshot::channel();
    let serve = axum::serve(listener, build_router(state))
        .with_graceful_shutdown(async move {
            let _signal = http_shutdown_receiver.await;
        })
        .into_future();
    tokio::pin!(serve);
    tokio::select! {
        result = &mut serve => result.map_err(StartupError::from),
        () = shutdown::signal() => {
            api_shutdown.trigger();
            let _signal = http_shutdown.send(());
            match shutdown::within(serve.as_mut(), shutdown::HTTP_DRAIN_TIMEOUT).await {
                shutdown::BoundedOutcome::Completed(result) => result.map_err(StartupError::from),
                shutdown::BoundedOutcome::TimedOut => {
                    eprintln!("pepeaudio-api HTTP drain timed out; closing remaining connections");
                    Ok(())
                },
            }
        }
    }
}

fn development_catalog() -> StaticHrirPresetCatalog {
    StaticHrirPresetCatalog::new([
        development_preset("studio-neutral", "Studio Neutral"),
        development_preset("wide-hall", "Wide Hall"),
    ])
}

fn development_preset(id: &str, display_name: &str) -> HrirPresetSummary {
    HrirPresetSummary {
        id: HrirPresetId::new(id).expect("hard-coded development preset ID is valid"),
        display_name: display_name.into(),
        source: HrirSourceMetadata {
            license_name: Some("Development fixture".into()),
            source_url: None,
            attribution: Some("Metadata only; replace with an imported production asset.".into()),
        },
    }
}

fn empty_snapshot(guild_id: GuildId) -> PlayerSnapshot {
    PlayerSnapshot {
        guild_id,
        voice_channel_id: None,
        revision: StateRevision::INITIAL,
        state: PlayerState::Disconnected,
        current_track: None,
        queued_tracks: 0,
        upcoming_tracks: Vec::new(),
        has_previous_track: false,
        volume: Volume::DEFAULT,
        repeat_mode: RepeatMode::Off,
        shuffle_enabled: false,
        hrir_preset: None,
        spatial_audio_enabled: false,
        observed_at: UnixTimeMillis::new(0),
    }
}

fn required(name: &'static str) -> Result<String, StartupError> {
    env::var(name).map_err(|_| StartupError::MissingEnvironment(name))
}

fn env_or(name: &'static str, default: &'static str) -> String {
    env::var(name).unwrap_or_else(|_| default.to_owned())
}
