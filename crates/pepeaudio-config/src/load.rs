use std::{num::NonZeroU32, time::Duration};

use pepeaudio_core::Volume;

use crate::{
    AppConfig, BotRuntimeConfig, ConfigError, ConfigResult, ConfigSource, DiscordConfig,
    PlayerLimits, SecretString, ServiceConfig, ShardConfig, ToolConfig,
    catalog::load_catalog,
    validate::{
        connection_url, http_url, instance_id, invalid, keyspace, nonempty_path, nonzero_u64,
        socket_addr,
    },
};

pub(crate) fn load(source: &impl ConfigSource) -> ConfigResult<AppConfig> {
    let player = load_player_limits(source)?;
    let tools = load_tools(source)?;
    let catalog = load_catalog(source, player.max_playlist_items)?;
    validate_catalog_tools(&catalog, &tools)?;
    Ok(AppConfig {
        discord: load_discord(source)?,
        shards: load_shards(source)?,
        instance_id: load_instance_id(source)?,
        services: load_services(source)?,
        player,
        tools,
        catalog,
    })
}

pub(crate) fn load_bot_runtime(source: &impl ConfigSource) -> ConfigResult<BotRuntimeConfig> {
    let player = load_player_limits(source)?;
    let tools = load_tools(source)?;
    let catalog = load_catalog(source, player.max_playlist_items)?;
    validate_catalog_tools(&catalog, &tools)?;
    Ok(BotRuntimeConfig {
        instance_id: load_instance_id(source)?,
        shards: load_shards(source)?,
        database_url: load_database_url(source)?,
        valkey_url: load_valkey_url(source)?,
        valkey_keyspace: keyspace(
            "PEPEAUDIO_VALKEY_KEYSPACE",
            required(source, "PEPEAUDIO_VALKEY_KEYSPACE")?,
        )?,
        player,
        tools,
        catalog,
    })
}

fn validate_catalog_tools(catalog: &crate::CatalogConfig, tools: &ToolConfig) -> ConfigResult<()> {
    if catalog.cross_service_matching_enabled && !tools.site_extractors_enabled {
        return Err(ConfigError::Inconsistent {
            reason: "cross-service matching requires site extractors",
        });
    }
    Ok(())
}

fn load_discord(source: &impl ConfigSource) -> ConfigResult<DiscordConfig> {
    Ok(DiscordConfig {
        token: secret(
            source,
            "PEPEAUDIO_DISCORD_TOKEN",
            "PEPEAUDIO_DISCORD_TOKEN_FILE",
            20,
        )?,
        application_id: nonzero_u64(
            "PEPEAUDIO_DISCORD_APPLICATION_ID",
            &required(source, "PEPEAUDIO_DISCORD_APPLICATION_ID")?,
        )?,
        client_id: nonzero_u64(
            "PEPEAUDIO_DISCORD_CLIENT_ID",
            &required(source, "PEPEAUDIO_DISCORD_CLIENT_ID")?,
        )?,
        client_secret: secret(
            source,
            "PEPEAUDIO_DISCORD_CLIENT_SECRET",
            "PEPEAUDIO_DISCORD_CLIENT_SECRET_FILE",
            1,
        )?,
        oauth_redirect_url: http_url(
            "PEPEAUDIO_DISCORD_OAUTH_REDIRECT_URL",
            &required(source, "PEPEAUDIO_DISCORD_OAUTH_REDIRECT_URL")?,
        )?,
    })
}

fn load_shards(source: &impl ConfigSource) -> ConfigResult<ShardConfig> {
    let shard_total = parse::<NonZeroU32>(source, "PEPEAUDIO_SHARD_TOTAL")?;
    let shard_start = parse::<u32>(source, "PEPEAUDIO_SHARD_START")?;
    let shard_end = optional_parse(source, "PEPEAUDIO_SHARD_END_EXCLUSIVE")?
        .unwrap_or_else(|| shard_total.get());
    ShardConfig::new(shard_total.get(), shard_start, shard_end).map_err(|_| {
        ConfigError::Inconsistent {
            reason: "shard range must be non-empty and contained in SHARD_TOTAL",
        }
    })
}

fn load_instance_id(source: &impl ConfigSource) -> ConfigResult<String> {
    instance_id(
        "PEPEAUDIO_INSTANCE_ID",
        required(source, "PEPEAUDIO_INSTANCE_ID")?,
    )
}

fn load_services(source: &impl ConfigSource) -> ConfigResult<ServiceConfig> {
    Ok(ServiceConfig {
        database_url: load_database_url(source)?,
        valkey_url: load_valkey_url(source)?,
        public_base_url: http_url(
            "PEPEAUDIO_PUBLIC_BASE_URL",
            &required(source, "PEPEAUDIO_PUBLIC_BASE_URL")?,
        )?,
        api_bind: socket_addr(
            "PEPEAUDIO_API_BIND",
            &required(source, "PEPEAUDIO_API_BIND")?,
        )?,
        session_cookie_secure: parse_bool(source, "PEPEAUDIO_SESSION_COOKIE_SECURE")?,
        session_key: secret(
            source,
            "PEPEAUDIO_SESSION_KEY",
            "PEPEAUDIO_SESSION_KEY_FILE",
            32,
        )?,
        valkey_keyspace: keyspace(
            "PEPEAUDIO_VALKEY_KEYSPACE",
            required(source, "PEPEAUDIO_VALKEY_KEYSPACE")?,
        )?,
    })
}

fn load_database_url(source: &impl ConfigSource) -> ConfigResult<SecretString> {
    let value = sensitive_required(
        source,
        "PEPEAUDIO_DATABASE_URL",
        "PEPEAUDIO_DATABASE_URL_FILE",
    )?;
    connection_url(
        "PEPEAUDIO_DATABASE_URL",
        &value,
        &["postgres", "postgresql"],
        true,
    )?;
    Ok(SecretString::new(value))
}

fn load_valkey_url(source: &impl ConfigSource) -> ConfigResult<SecretString> {
    let value = sensitive_required(source, "PEPEAUDIO_VALKEY_URL", "PEPEAUDIO_VALKEY_URL_FILE")?;
    connection_url("PEPEAUDIO_VALKEY_URL", &value, &["redis", "rediss"], false)?;
    Ok(SecretString::new(value))
}

fn load_player_limits(source: &impl ConfigSource) -> ConfigResult<PlayerLimits> {
    let idle_seconds = bounded::<u64>(source, "PEPEAUDIO_IDLE_DISCONNECT_SECONDS", 30, 86_400)?;
    let default_volume = parse::<u8>(source, "PEPEAUDIO_DEFAULT_VOLUME_PERCENT")?;
    let default_volume = Volume::new(default_volume).map_err(|_| {
        invalid(
            "PEPEAUDIO_DEFAULT_VOLUME_PERCENT",
            "must be between 0 and 100",
        )
    })?;
    let max_upload_bytes = std::num::NonZeroU64::new(bounded::<u64>(
        source,
        "PEPEAUDIO_MAX_UPLOAD_BYTES",
        1_024,
        1_073_741_824,
    )?)
    .expect("lower bound guarantees non-zero");
    let max_managed_media_bytes = optional_nonzero_u64(
        source,
        "PEPEAUDIO_MAX_MANAGED_MEDIA_BYTES",
        10 * 1024 * 1024 * 1024,
    )?;
    let max_site_media_bytes =
        optional_nonzero_u64(source, "PEPEAUDIO_MAX_SITE_MEDIA_BYTES", 100 * 1024 * 1024)?;
    if max_upload_bytes > max_managed_media_bytes || max_site_media_bytes > max_managed_media_bytes
    {
        return Err(ConfigError::Inconsistent {
            reason: "per-media byte limits must not exceed MAX_MANAGED_MEDIA_BYTES",
        });
    }
    let max_queue_items = bounded_nonzero_u32(source, "PEPEAUDIO_MAX_QUEUE_ITEMS", 1, 100)?;
    let configured_playlist =
        optional_bounded_u32(source, "PEPEAUDIO_MAX_PLAYLIST_ITEMS", 25, 1, 100)?;
    let max_playlist_items = NonZeroU32::new(configured_playlist.min(max_queue_items.get()))
        .expect("both bounds are non-zero");
    Ok(PlayerLimits {
        idle_disconnect: Duration::from_secs(idle_seconds),
        default_volume,
        // Upcoming items are part of each realtime snapshot. Keep the bound
        // below the Web client's 1 MiB SSE frame budget even when validated
        // titles use their full 120-character allowance.
        max_queue_items,
        max_track_duration: Duration::from_secs(bounded::<u64>(
            source,
            "PEPEAUDIO_MAX_TRACK_DURATION_SECONDS",
            1,
            604_800,
        )?),
        max_upload_bytes,
        max_site_media_bytes,
        max_playlist_items,
        max_managed_media_bytes,
    })
}

fn optional_nonzero_u64(
    source: &impl ConfigSource,
    name: &'static str,
    default: u64,
) -> ConfigResult<std::num::NonZeroU64> {
    match source.get(name)? {
        Some(value) => value
            .parse::<std::num::NonZeroU64>()
            .map_err(|_| invalid(name, "must be a positive 64-bit integer")),
        None => Ok(std::num::NonZeroU64::new(default).expect("default is non-zero")),
    }
}

fn load_tools(source: &impl ConfigSource) -> ConfigResult<ToolConfig> {
    let tools = ToolConfig {
        ffmpeg_path: nonempty_path(
            "PEPEAUDIO_FFMPEG_PATH",
            required(source, "PEPEAUDIO_FFMPEG_PATH")?,
        )?,
        ffprobe_path: nonempty_path(
            "PEPEAUDIO_FFPROBE_PATH",
            required(source, "PEPEAUDIO_FFPROBE_PATH")?,
        )?,
        hrir_directory: nonempty_path(
            "PEPEAUDIO_HRIR_DIRECTORY",
            required(source, "PEPEAUDIO_HRIR_DIRECTORY")?,
        )?,
        upload_directory: nonempty_path(
            "PEPEAUDIO_UPLOAD_DIRECTORY",
            required(source, "PEPEAUDIO_UPLOAD_DIRECTORY")?,
        )?,
        site_extractors_enabled: parse_bool(source, "PEPEAUDIO_ENABLE_SITE_EXTRACTORS")?,
        ytdlp_path: nonempty_path(
            "PEPEAUDIO_YTDLP_PATH",
            required(source, "PEPEAUDIO_YTDLP_PATH")?,
        )?,
        deno_path: optional_path(source, "PEPEAUDIO_DENO_PATH", "deno")?,
        deno_directory: optional_path(source, "PEPEAUDIO_DENO_DIR", "/tmp/pepeaudio-deno")?,
    };
    if tools.hrir_directory == tools.upload_directory {
        return Err(ConfigError::Inconsistent {
            reason: "HRIR and upload directories must be different",
        });
    }
    Ok(tools)
}

fn optional_path(
    source: &impl ConfigSource,
    name: &'static str,
    default: &'static str,
) -> ConfigResult<std::path::PathBuf> {
    nonempty_path(
        name,
        source.get(name)?.unwrap_or_else(|| default.to_owned()),
    )
}

pub(crate) fn required(source: &impl ConfigSource, name: &'static str) -> ConfigResult<String> {
    source.get(name)?.ok_or(ConfigError::Missing { name })
}

pub(crate) fn secret(
    source: &impl ConfigSource,
    name: &'static str,
    file_name: &'static str,
    minimum_bytes: usize,
) -> ConfigResult<SecretString> {
    let value = sensitive_required(source, name, file_name)?;
    if value.len() < minimum_bytes
        || value.chars().all(char::is_whitespace)
        || value == "replace-me"
        || value.starts_with("replace-with-")
    {
        return Err(invalid(name, "does not satisfy the minimum secret length"));
    }
    Ok(SecretString::new(value))
}

pub(crate) fn sensitive_required(
    source: &impl ConfigSource,
    name: &'static str,
    file_name: &'static str,
) -> ConfigResult<String> {
    match (source.get(name)?, source.get(file_name)?) {
        (Some(_), Some(_)) => Err(ConfigError::ConflictingSecretSources { name, file_name }),
        (Some(value), None) => Ok(value),
        (None, Some(path)) => read_secret_file(file_name, &path),
        (None, None) => Err(ConfigError::MissingSecret { name, file_name }),
    }
}

fn read_secret_file(name: &'static str, path: &str) -> ConfigResult<String> {
    if path.trim().is_empty() || path.contains('\0') {
        return Err(invalid(name, "must identify one readable file"));
    }
    let mut value = std::fs::read_to_string(path).map_err(|_| ConfigError::SecretFile { name })?;
    if value.ends_with("\r\n") {
        value.truncate(value.len() - 2);
    } else if value.ends_with('\n') {
        value.pop();
    }
    Ok(value)
}

pub(crate) fn parse<T>(source: &impl ConfigSource, name: &'static str) -> ConfigResult<T>
where
    T: std::str::FromStr,
{
    required(source, name)?
        .parse::<T>()
        .map_err(|_| invalid(name, "has an invalid value"))
}

fn optional_parse<T>(source: &impl ConfigSource, name: &'static str) -> ConfigResult<Option<T>>
where
    T: std::str::FromStr,
{
    source
        .get(name)?
        .map(|value| {
            value
                .parse::<T>()
                .map_err(|_| invalid(name, "has an invalid value"))
        })
        .transpose()
}

fn parse_bool(source: &impl ConfigSource, name: &'static str) -> ConfigResult<bool> {
    match required(source, name)?.as_str() {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(invalid(name, "must be exactly true or false")),
    }
}

pub(crate) fn bounded<T>(
    source: &impl ConfigSource,
    name: &'static str,
    minimum: T,
    maximum: T,
) -> ConfigResult<T>
where
    T: Copy + Ord + std::str::FromStr,
{
    let value = parse::<T>(source, name)?;
    if value < minimum || value > maximum {
        Err(invalid(name, "is outside the supported range"))
    } else {
        Ok(value)
    }
}

fn bounded_nonzero_u32(
    source: &impl ConfigSource,
    name: &'static str,
    minimum: u32,
    maximum: u32,
) -> ConfigResult<NonZeroU32> {
    NonZeroU32::new(bounded::<u32>(source, name, minimum, maximum)?)
        .ok_or_else(|| invalid(name, "must be non-zero"))
}

fn optional_bounded_u32(
    source: &impl ConfigSource,
    name: &'static str,
    default: u32,
    minimum: u32,
    maximum: u32,
) -> ConfigResult<u32> {
    let value = optional_parse(source, name)?.unwrap_or(default);
    if value < minimum || value > maximum {
        Err(invalid(name, "is outside the supported range"))
    } else {
        Ok(value)
    }
}
