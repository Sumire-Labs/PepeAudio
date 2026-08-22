use std::{sync::Arc, time::Duration};

use pepeaudio_catalog::{
    AppleMusicCatalog, AppleMusicPublicCatalog, CatalogResolver, CatalogResolverBuilder,
    SpotifyCatalog, SpotifyPublicCatalog,
};
use pepeaudio_config::BotRuntimeConfig;
use pepeaudio_media::ManagedDownloadJanitor;
use pepeaudio_pipeline::{
    FfmpegDecoderFactory, HrirProvider, LookupHrirProvider, ManagedMediaResolver, TrackResolver,
};
use pepeaudio_presets::{CatalogLimits, HrirCatalog};
use pepeaudio_runtime::{
    GuildPresenceRuntime, SettingsPersistenceRuntime, SnapshotPublisherRuntime,
};
use pepeaudio_storage::{
    HrirChannelLayout, HrirPresetMetadata, Keyspace, PostgresStorage, ValkeyStore,
};
use serenity::http::Http;
use songbird::Songbird;

// The shared-FDL partitioned backend processes a 9,600-frame (200 ms at 48 kHz)
// horizontal-orbit IR at 248.9x realtime on the Windows 11 reference host.
// Crossfade doubles renderer work; multi-guild target-host admission remains an
// operational acceptance gate, so longer presets continue to fail closed.
const MAX_PARTITIONED_HRIR_FRAMES: usize = 9_600;
const SNAPSHOT_TTL: Duration = Duration::from_hours(24);

use crate::{
    BotConfig, BotData, BotError, ComponentIdCodec, ComponentsV2Responder, DiscordComponentsV2Rest,
    HrirOption, NowPanelUpdater, PlayerRegistry,
    guild_lifecycle::GuildLifecycleRuntime,
    guild_policy::PostgresGuildPolicy,
    production_factory::ProductionPlayerFactory,
    production_media::ProductionMediaResolver,
    production_media_lifecycle::{PreparedMedia, prepare_managed_media},
};

pub(crate) struct ProductionServices {
    pub(crate) valkey: ValkeyStore,
    pub(crate) postgres: PostgresStorage,
    pub(crate) manager: Arc<Songbird>,
    pub(crate) players: Arc<PlayerRegistry>,
    pub(crate) snapshots: SnapshotPublisherRuntime<ValkeyStore>,
    pub(crate) settings_persistence: SettingsPersistenceRuntime<PostgresStorage>,
    pub(crate) guild_lifecycle: GuildLifecycleRuntime,
    pub(crate) presence: GuildPresenceRuntime,
    pub(crate) media_janitor: Arc<ManagedDownloadJanitor>,
    pub(crate) data: BotData,
}

struct StorageServices {
    valkey: ValkeyStore,
    snapshots: SnapshotPublisherRuntime<ValkeyStore>,
    postgres: PostgresStorage,
}

pub(crate) async fn assemble(
    discord: &BotConfig,
    runtime: &BotRuntimeConfig,
) -> Result<ProductionServices, BotError> {
    let prepared_media = prepare_managed_media(runtime).await?;
    let StorageServices {
        valkey,
        snapshots,
        postgres,
    } = connect_storage(runtime).await?;
    let catalog = load_catalog(runtime)?;
    synchronize_catalog(&postgres, &catalog).await?;
    let hrir_options = hrir_options(&catalog);
    let lookup_catalog = catalog.clone();
    let hrirs: Arc<dyn HrirProvider> = Arc::new(LookupHrirProvider::new(
        move |identifier: &pepeaudio_core::HrirPresetId| lookup_catalog.get(identifier),
    ));
    let resolver: Arc<dyn TrackResolver> = Arc::new(
        ManagedMediaResolver::new(&prepared_media.tools.upload_directory)
            .await
            .map_err(|_| BotError::MediaAdapter)?,
    );
    let decoder = Arc::new(
        FfmpegDecoderFactory::new(
            &runtime.tools.ffmpeg_path,
            16,
            Duration::from_secs(10),
            Duration::from_secs(10),
            64 * 1024,
            runtime.player.max_track_duration,
        )
        .map_err(|_| BotError::MediaAdapter)?,
    );
    let manager = Songbird::serenity();
    let settings_persistence = SettingsPersistenceRuntime::start(postgres.clone());
    let factory = ProductionPlayerFactory::new(
        manager.clone(),
        resolver,
        decoder,
        hrirs,
        catalog,
        postgres.clone(),
        valkey.clone(),
        snapshots.handle(),
        settings_persistence.handle(),
        runtime.player.clone(),
    )
    .map_err(|_| BotError::PlayerFactory)?;
    let players = Arc::new(PlayerRegistry::new(Arc::new(factory)));
    let media = build_media_resolver(runtime, &prepared_media).await?;
    let presence = GuildPresenceRuntime::start(
        valkey.clone(),
        runtime.instance_id.clone(),
        Duration::from_secs(30),
        Duration::from_secs(10),
    )
    .map_err(|_| BotError::RuntimeDependency)?;
    let guild_lifecycle = GuildLifecycleRuntime::start(valkey.clone(), presence.handle());
    let components: Arc<dyn ComponentsV2Responder> = Arc::new(DiscordComponentsV2Rest::new(
        Arc::new(Http::new(&discord.discord_token)),
    ));
    let component_ids = ComponentIdCodec::new(&discord.component_signing_key)
        .expect("BotConfig validates the component signing key");
    let now_panels = NowPanelUpdater::new(
        components.clone(),
        component_ids.clone(),
        hrir_options.clone(),
    );
    let data = BotData {
        players: players.clone(),
        media,
        components,
        component_ids,
        hrir_options,
        now_panels,
        guild_policy: Arc::new(PostgresGuildPolicy::new(postgres.clone())),
        guild_lifecycle: Some(guild_lifecycle.handle()),
    };
    Ok(ProductionServices {
        valkey,
        postgres,
        manager,
        players,
        snapshots,
        settings_persistence,
        guild_lifecycle,
        presence,
        media_janitor: prepared_media.janitor,
        data,
    })
}

async fn build_media_resolver(
    runtime: &BotRuntimeConfig,
    prepared: &PreparedMedia,
) -> Result<Arc<ProductionMediaResolver>, BotError> {
    let media = ProductionMediaResolver::new(
        &prepared.tools,
        runtime.player.max_upload_bytes.get(),
        runtime.player.max_site_media_bytes.get(),
        runtime.player.max_track_duration,
        usize::try_from(runtime.player.max_playlist_items.get())
            .map_err(|_| BotError::MediaAdapter)?,
        usize::try_from(runtime.player.max_queue_items.get())
            .map_err(|_| BotError::MediaAdapter)?,
        prepared.leases.clone(),
        prepared.janitor.clone(),
        build_catalog_resolver(runtime)?,
    )
    .map_err(|_| BotError::MediaAdapter)?;
    media
        .verify_site_tools()
        .await
        .map_err(|_| BotError::MediaAdapter)?;
    Ok(Arc::new(media))
}

fn build_catalog_resolver(runtime: &BotRuntimeConfig) -> Result<Option<CatalogResolver>, BotError> {
    if !runtime.catalog.cross_service_matching_enabled {
        return Ok(None);
    }
    let mut builder = CatalogResolverBuilder::new()
        .collection_limit(
            usize::try_from(runtime.catalog.max_items.get()).map_err(|_| BotError::MediaAdapter)?,
        )
        .map_err(|_| BotError::MediaAdapter)?;
    let (spotify_mode, apple_music_mode) = catalog_provider_modes(
        CatalogProviderSelection {
            credentials_configured: runtime.catalog.spotify.is_some(),
            public_metadata_enabled: runtime.catalog.spotify_public_metadata_enabled,
        },
        CatalogProviderSelection {
            credentials_configured: runtime.catalog.apple_music.is_some(),
            public_metadata_enabled: runtime.catalog.apple_music_public_metadata_enabled,
        },
    );
    match spotify_mode {
        CatalogProviderMode::Credentials => {
            let spotify = runtime
                .catalog
                .spotify
                .as_ref()
                .expect("credential mode requires Spotify configuration");
            let provider = SpotifyCatalog::new(
                spotify.client_id.expose_secret().to_owned(),
                spotify.client_secret.expose_secret().to_owned(),
                spotify.market.clone(),
            )
            .map_err(|_| BotError::MediaAdapter)?;
            builder = builder.spotify(provider);
        }
        CatalogProviderMode::PublicMetadata => {
            let provider = SpotifyPublicCatalog::new().map_err(|_| BotError::MediaAdapter)?;
            builder = builder.spotify_public(provider);
        }
        CatalogProviderMode::Disabled => {}
    }
    match apple_music_mode {
        CatalogProviderMode::Credentials => {
            let apple = runtime
                .catalog
                .apple_music
                .as_ref()
                .expect("credential mode requires Apple Music configuration");
            let provider = AppleMusicCatalog::new(
                apple.team_id.clone(),
                apple.key_id.clone(),
                apple.private_key.expose_secret(),
            )
            .map_err(|_| BotError::MediaAdapter)?;
            builder = builder.apple_music(provider);
        }
        CatalogProviderMode::PublicMetadata => {
            let provider = AppleMusicPublicCatalog::new().map_err(|_| BotError::MediaAdapter)?;
            builder = builder.apple_music_public(provider);
        }
        CatalogProviderMode::Disabled => {}
    }
    Ok(Some(builder.build()))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CatalogProviderMode {
    Disabled,
    PublicMetadata,
    Credentials,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CatalogProviderSelection {
    credentials_configured: bool,
    public_metadata_enabled: bool,
}

const fn provider_mode(selection: CatalogProviderSelection) -> CatalogProviderMode {
    if selection.credentials_configured {
        CatalogProviderMode::Credentials
    } else if selection.public_metadata_enabled {
        CatalogProviderMode::PublicMetadata
    } else {
        CatalogProviderMode::Disabled
    }
}

const fn catalog_provider_modes(
    spotify: CatalogProviderSelection,
    apple_music: CatalogProviderSelection,
) -> (CatalogProviderMode, CatalogProviderMode) {
    (provider_mode(spotify), provider_mode(apple_music))
}

async fn connect_storage(runtime: &BotRuntimeConfig) -> Result<StorageServices, BotError> {
    let keyspace =
        Keyspace::new(runtime.valkey_keyspace.clone()).map_err(|_| BotError::StorageDependency)?;
    let valkey = ValkeyStore::connect(runtime.valkey_url.expose_secret(), keyspace)
        .await
        .map_err(|_| BotError::StorageDependency)?;
    let snapshots = SnapshotPublisherRuntime::start(valkey.clone(), SNAPSHOT_TTL)
        .map_err(|_| BotError::RuntimeDependency)?;
    let postgres = PostgresStorage::connect(runtime.database_url.expose_secret(), 8)
        .await
        .map_err(|_| BotError::StorageDependency)?;
    postgres
        .ping()
        .await
        .map_err(|_| BotError::StorageDependency)?;
    Ok(StorageServices {
        valkey,
        snapshots,
        postgres,
    })
}

fn load_catalog(runtime: &BotRuntimeConfig) -> Result<HrirCatalog, BotError> {
    // DirectFir remains the numerical oracle. Production uses the measured
    // partitioned backend but still rejects unbenchmarked IR lengths.
    let limits = CatalogLimits::new(25, 64 * 1024 * 1024, MAX_PARTITIONED_HRIR_FRAMES)
        .and_then(|limits| limits.with_prepared_frame_limit(MAX_PARTITIONED_HRIR_FRAMES))
        .map_err(|_| BotError::HrirCatalog)?;
    let catalog = HrirCatalog::load(&runtime.tools.hrir_directory, limits)
        .map_err(|_| BotError::HrirCatalog)?;
    if catalog.is_empty() {
        Err(BotError::HrirCatalog)
    } else {
        Ok(catalog)
    }
}

fn hrir_options(catalog: &HrirCatalog) -> Arc<[HrirOption]> {
    catalog
        .descriptors()
        .into_iter()
        .map(|descriptor| HrirOption {
            id: descriptor.id.to_string(),
            label: descriptor.display_name,
            description: descriptor.description,
        })
        .collect::<Vec<_>>()
        .into()
}

async fn synchronize_catalog(
    postgres: &PostgresStorage,
    catalog: &HrirCatalog,
) -> Result<(), BotError> {
    let presets: Vec<_> = catalog
        .descriptors()
        .into_iter()
        .map(|descriptor| HrirPresetMetadata {
            preset_id: descriptor.id,
            owner_guild_id: None,
            display_name: descriptor.display_name,
            description: descriptor.description,
            storage_key: descriptor.storage_key,
            sha256_hex: descriptor.sha256_hex,
            sample_rate: descriptor.source_sample_rate_hz,
            channel_layout: match descriptor.source_layout {
                pepeaudio_hrir::SourceLayout::SevenChannelMirrored => HrirChannelLayout::Hesuvi7,
                pepeaudio_hrir::SourceLayout::FourteenChannelIndependent => {
                    HrirChannelLayout::Hesuvi14
                }
            },
            file_size_bytes: descriptor.file_size_bytes,
            license_name: None,
            license_url: None,
            attribution: None,
            created_at: time::OffsetDateTime::UNIX_EPOCH,
        })
        .collect();
    postgres
        .synchronize_global_hrir_presets(&presets)
        .await
        .map_err(|_| BotError::HrirCatalog)
}

#[cfg(test)]
mod tests {
    use super::{CatalogProviderMode, CatalogProviderSelection, catalog_provider_modes};

    const DISABLED: CatalogProviderSelection = CatalogProviderSelection {
        credentials_configured: false,
        public_metadata_enabled: false,
    };
    const PUBLIC: CatalogProviderSelection = CatalogProviderSelection {
        credentials_configured: false,
        public_metadata_enabled: true,
    };
    const CREDENTIALS: CatalogProviderSelection = CatalogProviderSelection {
        credentials_configured: true,
        public_metadata_enabled: false,
    };

    #[test]
    fn spotify_credentials_do_not_enable_apple_public_metadata() {
        assert_eq!(
            catalog_provider_modes(CREDENTIALS, DISABLED),
            (
                CatalogProviderMode::Credentials,
                CatalogProviderMode::Disabled
            )
        );
    }

    #[test]
    fn apple_credentials_do_not_enable_spotify_public_metadata() {
        assert_eq!(
            catalog_provider_modes(DISABLED, CREDENTIALS),
            (
                CatalogProviderMode::Disabled,
                CatalogProviderMode::Credentials
            )
        );
    }

    #[test]
    fn public_metadata_mode_explicitly_enables_both_providers() {
        assert_eq!(
            catalog_provider_modes(PUBLIC, PUBLIC),
            (
                CatalogProviderMode::PublicMetadata,
                CatalogProviderMode::PublicMetadata
            )
        );
    }
}
