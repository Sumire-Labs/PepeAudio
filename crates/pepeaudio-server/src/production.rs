use std::{sync::Arc, time::Duration};

use pepeaudio_api::{
    ApiConfig, ApiShutdown, AppState, CompositeReadiness, DEFAULT_COMMAND_TTL, ReadinessProbe,
    SessionAuthenticator, SystemClock, build_router,
};
use pepeaudio_auth::{
    AuthConfig, AuthService, DiscordOAuthClient, DiscordOAuthConfig, SecretString,
    SessionGuildAuthorizer, SessionPolicy, SystemAuthClock, ValkeyAuthStore, build_auth_router,
};
use pepeaudio_config::ApiRuntimeConfig;
use pepeaudio_runtime::ValkeyApiBackend;
use pepeaudio_storage::{Keyspace, PostgresStorage, ValkeyStore};

use crate::{
    error::StartupError,
    hrir_catalog::PostgresHrirPresetCatalog,
    production_lifecycle,
    readiness::{AuthReadiness, PostgresReadiness, shared_presence},
};

pub(crate) async fn run() -> Result<(), StartupError> {
    let config = ApiRuntimeConfig::from_env()?;
    let keyspace = Keyspace::new(config.valkey_keyspace.clone())
        .map_err(|_| StartupError::StorageDependency)?;
    let valkey = ValkeyStore::connect(config.valkey_url.expose_secret(), keyspace)
        .await
        .map_err(|_| StartupError::StorageDependency)?;
    let postgres = PostgresStorage::connect(config.database_url.expose_secret(), 8)
        .await
        .map_err(|_| StartupError::StorageDependency)?;

    let auth_config = AuthConfig::new(
        DiscordOAuthConfig::new(
            config.discord_client_id.get().to_string(),
            SecretString::new(config.discord_client_secret.expose_secret()),
            config.discord_oauth_redirect_url.clone(),
        )?,
        SessionPolicy::new(
            config.session_absolute_ttl,
            config.session_idle_ttl,
            config.oauth_state_ttl,
        )?,
        config.valkey_keyspace.clone(),
        config.auth_success_path.clone(),
    )?;
    let auth_store = ValkeyAuthStore::connect(config.valkey_url.expose_secret(), &auth_config)
        .await
        .map_err(|_| StartupError::AuthDependency)?;
    let discord = DiscordOAuthClient::new(auth_config.discord().clone())
        .map_err(|_| StartupError::AuthDependency)?;
    let presence = shared_presence(valkey.clone());
    let auth_service = AuthService::with_discord_client(
        auth_config,
        discord,
        Arc::new(auth_store.clone()),
        Arc::new(auth_store.clone()),
        presence.clone(),
        Arc::new(SystemAuthClock),
    );

    let (backend, backend_runtime) =
        ValkeyApiBackend::start(valkey, config.shard_total, 256, Duration::from_secs(1))
            .await
            .map_err(|_| StartupError::RuntimeDependency)?;
    let readiness = CompositeReadiness::new([
        backend.clone() as Arc<dyn ReadinessProbe>,
        Arc::new(PostgresReadiness(postgres.clone())),
        Arc::new(AuthReadiness(auth_store.clone())),
    ]);
    let api_config = ApiConfig::new(
        config.public_base_url.as_str().trim_end_matches('/'),
        DEFAULT_COMMAND_TTL,
    )?;
    let api_shutdown = ApiShutdown::new();
    let state = AppState::new(
        api_config,
        Arc::new(SessionAuthenticator::new(auth_store.clone())),
        Arc::new(SessionGuildAuthorizer::new(Arc::new(auth_store), presence)),
        backend.clone(),
        Arc::new(PostgresHrirPresetCatalog::new(postgres.clone())),
        backend.clone(),
        backend.clone(),
        backend.clone(),
        Arc::new(readiness),
        Arc::new(SystemClock),
    )
    .with_shutdown(api_shutdown.clone());
    let app = build_router(state).merge(build_auth_router(auth_service));
    let listener = tokio::net::TcpListener::bind(config.api_bind).await?;
    eprintln!(
        "pepeaudio-api production server listening on {}",
        config.api_bind
    );
    production_lifecycle::serve(listener, app, api_shutdown, backend_runtime, postgres).await
}
