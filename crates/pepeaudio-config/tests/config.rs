use std::time::Duration;

use pepeaudio_config::{AppConfig, BotRuntimeConfig, ConfigError, MapSource};

fn valid_source() -> MapSource {
    MapSource::new()
        .with("PEPEAUDIO_DISCORD_TOKEN", "token-value-that-is-long-enough")
        .with("PEPEAUDIO_DISCORD_APPLICATION_ID", "123")
        .with("PEPEAUDIO_DISCORD_CLIENT_ID", "123")
        .with("PEPEAUDIO_DISCORD_CLIENT_SECRET", "oauth-secret")
        .with(
            "PEPEAUDIO_DISCORD_OAUTH_REDIRECT_URL",
            "http://localhost:8080/auth/discord/callback",
        )
        .with("PEPEAUDIO_SHARD_TOTAL", "4")
        .with("PEPEAUDIO_SHARD_START", "1")
        .with("PEPEAUDIO_SHARD_END_EXCLUSIVE", "3")
        .with("PEPEAUDIO_INSTANCE_ID", "local-1")
        .with(
            "PEPEAUDIO_DATABASE_URL",
            "postgres://app:password@localhost:5432/pepeaudio",
        )
        .with(
            "PEPEAUDIO_VALKEY_URL",
            "redis://default:password@localhost:6379/0",
        )
        .with("PEPEAUDIO_PUBLIC_BASE_URL", "http://localhost:8080")
        .with("PEPEAUDIO_API_BIND", "127.0.0.1:3000")
        .with("PEPEAUDIO_SESSION_COOKIE_SECURE", "false")
        .with(
            "PEPEAUDIO_SESSION_KEY",
            "a-development-session-key-with-32-bytes",
        )
        .with("PEPEAUDIO_VALKEY_KEYSPACE", "pepeaudio-test")
        .with("PEPEAUDIO_IDLE_DISCONNECT_SECONDS", "300")
        .with("PEPEAUDIO_DEFAULT_VOLUME_PERCENT", "75")
        .with("PEPEAUDIO_MAX_QUEUE_ITEMS", "100")
        .with("PEPEAUDIO_MAX_TRACK_DURATION_SECONDS", "21600")
        .with("PEPEAUDIO_MAX_UPLOAD_BYTES", "10485760")
        .with("PEPEAUDIO_MAX_MANAGED_MEDIA_BYTES", "10737418240")
        .with("PEPEAUDIO_FFMPEG_PATH", "ffmpeg")
        .with("PEPEAUDIO_FFPROBE_PATH", "ffprobe")
        .with("PEPEAUDIO_HRIR_DIRECTORY", "/app/assets/hrir")
        .with("PEPEAUDIO_UPLOAD_DIRECTORY", "/app/storage/uploads")
        .with("PEPEAUDIO_ENABLE_SITE_EXTRACTORS", "false")
        .with("PEPEAUDIO_YTDLP_PATH", "yt-dlp")
        .with("PEPEAUDIO_ENABLE_CROSS_SERVICE_MATCHING", "false")
        .with("PEPEAUDIO_CATALOG_MAX_ITEMS", "25")
}

#[test]
fn loads_every_documented_setting_into_typed_sections() {
    let config = AppConfig::from_source(&valid_source()).expect("valid configuration");

    assert_eq!(config.discord.application_id.get(), 123);
    assert_eq!(config.shards.total.get(), 4);
    assert!(config.shards.owns(1));
    assert!(config.shards.owns(2));
    assert!(!config.shards.owns(3));
    assert_eq!(config.player.idle_disconnect, Duration::from_mins(5));
    assert_eq!(config.player.default_volume.percent(), 75);
    assert_eq!(config.player.max_queue_items.get(), 100);
    assert_eq!(config.player.max_playlist_items.get(), 25);
    assert_eq!(config.player.max_site_media_bytes.get(), 100 * 1024 * 1024);
    assert_eq!(config.player.max_managed_media_bytes.get(), 10_737_418_240);
    assert!(!config.tools.site_extractors_enabled);
    assert_eq!(config.tools.deno_path, std::path::Path::new("deno"));
    assert!(!config.catalog.cross_service_matching_enabled);
    assert!(!config.catalog.spotify_public_metadata_enabled);
    assert!(!config.catalog.apple_music_public_metadata_enabled);
}

#[test]
fn debug_output_redacts_all_connection_and_application_secrets() {
    let config = AppConfig::from_source(&valid_source()).expect("valid configuration");
    let debug = format!("{config:?}");

    assert!(!debug.contains("token-value"));
    assert!(!debug.contains("oauth-secret"));
    assert!(!debug.contains(":password@"));
    assert!(!debug.contains("development-session-key"));
    assert_eq!(debug.matches("[REDACTED]").count(), 5);
}

#[test]
fn errors_identify_secret_variable_without_repeating_its_value() {
    let mut source = valid_source();
    source.insert("PEPEAUDIO_DISCORD_TOKEN", "too-short-secret");

    let error = AppConfig::from_source(&source).expect_err("short token must fail");
    let rendered = error.to_string();
    assert!(rendered.contains("PEPEAUDIO_DISCORD_TOKEN"));
    assert!(!rendered.contains("too-short-secret"));
}

#[test]
fn published_secret_placeholders_are_rejected() {
    let packaged_example = include_str!("../../../secrets/discord_client_secret.txt.example");
    for placeholder in [
        "replace-me",
        "replace-with-discord-oauth-client-secret",
        packaged_example.trim(),
    ] {
        let mut source = valid_source();
        source.insert("PEPEAUDIO_DISCORD_CLIENT_SECRET", placeholder);

        let error = AppConfig::from_source(&source).expect_err("placeholder must fail closed");
        assert!(
            error
                .to_string()
                .contains("PEPEAUDIO_DISCORD_CLIENT_SECRET")
        );
        assert!(!format!("{error:?} {error}").contains(placeholder));
    }
}

#[test]
fn rejects_empty_or_out_of_bounds_shard_ranges() {
    for (start, end, expected_reason) in [
        ("2", "2", "contained in SHARD_TOTAL"),
        ("3", "5", "contained in SHARD_TOTAL"),
    ] {
        let mut source = valid_source();
        source.insert("PEPEAUDIO_SHARD_START", start);
        source.insert("PEPEAUDIO_SHARD_END_EXCLUSIVE", end);

        let error = AppConfig::from_source(&source).expect_err("invalid shard range");
        assert!(error.to_string().contains(expected_reason));
    }
}

#[test]
fn omitted_shard_end_owns_the_rest_of_a_single_process_topology() {
    let mut source = valid_source();
    source.insert("PEPEAUDIO_SHARD_START", "0");
    source.remove("PEPEAUDIO_SHARD_END_EXCLUSIVE");

    let config = BotRuntimeConfig::from_source(&source).expect("implicit shard end");

    assert_eq!(config.shards.total(), 4);
    assert_eq!(config.shards.range(), 0..4);
}

#[test]
fn rejects_public_url_credentials_and_connection_url_fragments() {
    let mut public = valid_source();
    public.insert(
        "PEPEAUDIO_PUBLIC_BASE_URL",
        "https://user:pass@example.test",
    );
    assert!(matches!(
        AppConfig::from_source(&public),
        Err(ConfigError::Invalid {
            name: "PEPEAUDIO_PUBLIC_BASE_URL",
            ..
        })
    ));

    let mut database = valid_source();
    database.insert(
        "PEPEAUDIO_DATABASE_URL",
        "postgres://app:secret@db/pepeaudio#fragment",
    );
    let error = AppConfig::from_source(&database).expect_err("fragment must fail");
    assert!(!error.to_string().contains("secret"));
}

#[test]
fn validates_player_limits_and_separates_storage_directories() {
    let mut volume = valid_source();
    volume.insert("PEPEAUDIO_DEFAULT_VOLUME_PERCENT", "101");
    assert!(AppConfig::from_source(&volume).is_err());

    let mut upload = valid_source();
    upload.insert("PEPEAUDIO_MAX_UPLOAD_BYTES", "0");
    assert!(AppConfig::from_source(&upload).is_err());

    let mut capacity = valid_source();
    capacity.insert("PEPEAUDIO_MAX_MANAGED_MEDIA_BYTES", "1048575");
    assert!(matches!(
        AppConfig::from_source(&capacity),
        Err(ConfigError::Inconsistent { .. })
    ));

    let mut zero_capacity = valid_source();
    zero_capacity.insert("PEPEAUDIO_MAX_MANAGED_MEDIA_BYTES", "0");
    assert!(AppConfig::from_source(&zero_capacity).is_err());

    let mut default_capacity = valid_source();
    default_capacity.remove("PEPEAUDIO_MAX_MANAGED_MEDIA_BYTES");
    assert_eq!(
        AppConfig::from_source(&default_capacity)
            .expect("default capacity")
            .player
            .max_managed_media_bytes
            .get(),
        10 * 1024 * 1024 * 1024
    );

    let mut directories = valid_source();
    directories.insert("PEPEAUDIO_UPLOAD_DIRECTORY", "/app/assets/hrir");
    assert!(matches!(
        AppConfig::from_source(&directories),
        Err(ConfigError::Inconsistent { .. })
    ));
}

#[test]
fn loads_a_secret_from_file_and_removes_one_line_ending() {
    let path = std::env::temp_dir().join(format!(
        "pepeaudio-config-secret-{}-{}.txt",
        std::process::id(),
        std::thread::current().name().unwrap_or("unnamed")
    ));
    std::fs::write(&path, "synthetic-token-value-that-is-long-enough\r\n")
        .expect("write synthetic secret file");

    let mut source = valid_source();
    source.remove("PEPEAUDIO_DISCORD_TOKEN");
    source.insert(
        "PEPEAUDIO_DISCORD_TOKEN_FILE",
        path.to_string_lossy().into_owned(),
    );
    let config = AppConfig::from_source(&source).expect("file-backed secret");
    std::fs::remove_file(path).expect("remove synthetic secret file");

    assert_eq!(
        config.discord.token.expose_secret(),
        "synthetic-token-value-that-is-long-enough"
    );
}

#[test]
fn rejects_competing_secret_sources_without_exposing_either_value() {
    let mut source = valid_source();
    source.insert(
        "PEPEAUDIO_DISCORD_TOKEN_FILE",
        "path-that-must-not-appear-in-errors",
    );

    let error = AppConfig::from_source(&source).expect_err("competing sources");
    let rendered = error.to_string();
    assert!(rendered.contains("PEPEAUDIO_DISCORD_TOKEN"));
    assert!(rendered.contains("PEPEAUDIO_DISCORD_TOKEN_FILE"));
    assert!(!rendered.contains("token-value-that-is-long-enough"));
    assert!(!rendered.contains("path-that-must-not-appear-in-errors"));
}

#[test]
fn bot_runtime_does_not_require_oauth_or_session_secrets() {
    let mut source = valid_source();
    for name in [
        "PEPEAUDIO_DISCORD_TOKEN",
        "PEPEAUDIO_DISCORD_APPLICATION_ID",
        "PEPEAUDIO_DISCORD_CLIENT_ID",
        "PEPEAUDIO_DISCORD_CLIENT_SECRET",
        "PEPEAUDIO_DISCORD_OAUTH_REDIRECT_URL",
        "PEPEAUDIO_PUBLIC_BASE_URL",
        "PEPEAUDIO_API_BIND",
        "PEPEAUDIO_SESSION_COOKIE_SECURE",
        "PEPEAUDIO_SESSION_KEY",
    ] {
        source.remove(name);
    }

    let config = BotRuntimeConfig::from_source(&source).expect("Bot-only configuration");
    assert_eq!(config.instance_id, "local-1");
    assert_eq!(config.shards.range(), 1..3);
    assert_eq!(config.player.max_queue_items.get(), 100);
    let debug = format!("{config:?}");
    assert!(!debug.contains(":password@"));
}

#[test]
fn queue_limit_preserves_the_realtime_snapshot_frame_budget() {
    let source = valid_source().with("PEPEAUDIO_MAX_QUEUE_ITEMS", "101");
    let error = BotRuntimeConfig::from_source(&source).expect_err("oversized queue rejected");
    assert!(error.to_string().contains("PEPEAUDIO_MAX_QUEUE_ITEMS"));
}

#[test]
fn disabled_catalog_matching_does_not_read_optional_secret_files() {
    let source = valid_source()
        .with("PEPEAUDIO_SPOTIFY_CLIENT_ID", "configured-but-disabled")
        .with(
            "PEPEAUDIO_SPOTIFY_CLIENT_SECRET_FILE",
            "this-file-must-not-be-read",
        )
        .with("PEPEAUDIO_APPLE_MUSIC_TEAM_ID", "ABCDE12345")
        .with(
            "PEPEAUDIO_APPLE_MUSIC_PRIVATE_KEY_FILE",
            "this-file-must-not-be-read-either",
        );

    let config = BotRuntimeConfig::from_source(&source).expect("disabled false path");

    assert!(config.catalog.spotify.is_none());
    assert!(config.catalog.apple_music.is_none());
}

#[test]
fn enabled_catalog_matching_allows_public_metadata_but_rejects_partial_credentials() {
    let no_provider = valid_source()
        .with("PEPEAUDIO_ENABLE_SITE_EXTRACTORS", "true")
        .with("PEPEAUDIO_ENABLE_CROSS_SERVICE_MATCHING", "true");
    assert!(matches!(
        BotRuntimeConfig::from_source(&no_provider),
        Err(ConfigError::Inconsistent { .. })
    ));

    let public = valid_source()
        .with("PEPEAUDIO_ENABLE_SITE_EXTRACTORS", "true")
        .with("PEPEAUDIO_ENABLE_CROSS_SERVICE_MATCHING", "true")
        .with("PEPEAUDIO_ENABLE_SPOTIFY_PUBLIC_METADATA", "true")
        .with("PEPEAUDIO_ENABLE_APPLE_MUSIC_PUBLIC_METADATA", "true");
    let config = BotRuntimeConfig::from_source(&public).expect("public metadata configuration");
    assert!(config.catalog.cross_service_matching_enabled);
    assert!(config.catalog.spotify_public_metadata_enabled);
    assert!(config.catalog.apple_music_public_metadata_enabled);
    assert!(config.catalog.spotify.is_none());
    assert!(config.catalog.apple_music.is_none());

    let partial = valid_source()
        .with("PEPEAUDIO_ENABLE_SITE_EXTRACTORS", "true")
        .with("PEPEAUDIO_ENABLE_CROSS_SERVICE_MATCHING", "true")
        .with("PEPEAUDIO_SPOTIFY_CLIENT_ID", "client-id");
    assert!(matches!(
        BotRuntimeConfig::from_source(&partial),
        Err(ConfigError::Inconsistent { .. })
    ));

    let no_site_tools = valid_source()
        .with("PEPEAUDIO_ENABLE_CROSS_SERVICE_MATCHING", "true")
        .with("PEPEAUDIO_SPOTIFY_CLIENT_ID", "client-id")
        .with("PEPEAUDIO_SPOTIFY_CLIENT_SECRET", "private-client-secret");
    assert!(matches!(
        BotRuntimeConfig::from_source(&no_site_tools),
        Err(ConfigError::Inconsistent { .. })
    ));
}

#[test]
fn credential_catalog_configuration_does_not_enable_the_other_public_provider() {
    let spotify = valid_source()
        .with("PEPEAUDIO_ENABLE_SITE_EXTRACTORS", "true")
        .with("PEPEAUDIO_ENABLE_CROSS_SERVICE_MATCHING", "true")
        .with("PEPEAUDIO_SPOTIFY_CLIENT_ID", "client-id")
        .with("PEPEAUDIO_SPOTIFY_CLIENT_SECRET", "client-secret");
    let spotify = BotRuntimeConfig::from_source(&spotify).expect("Spotify credentials");
    assert!(spotify.catalog.spotify.is_some());
    assert!(spotify.catalog.apple_music.is_none());
    assert!(!spotify.catalog.spotify_public_metadata_enabled);
    assert!(!spotify.catalog.apple_music_public_metadata_enabled);

    let apple = valid_source()
        .with("PEPEAUDIO_ENABLE_SITE_EXTRACTORS", "true")
        .with("PEPEAUDIO_ENABLE_CROSS_SERVICE_MATCHING", "true")
        .with("PEPEAUDIO_APPLE_MUSIC_TEAM_ID", "ABCDE12345")
        .with("PEPEAUDIO_APPLE_MUSIC_KEY_ID", "KEY1234567")
        .with("PEPEAUDIO_APPLE_MUSIC_PRIVATE_KEY", "x".repeat(64));
    let apple = BotRuntimeConfig::from_source(&apple).expect("Apple Music credentials");
    assert!(apple.catalog.spotify.is_none());
    assert!(apple.catalog.apple_music.is_some());
    assert!(!apple.catalog.spotify_public_metadata_enabled);
    assert!(!apple.catalog.apple_music_public_metadata_enabled);
}

#[test]
fn catalog_credentials_are_redacted_and_dual_sources_fail_closed() {
    let enabled = valid_source()
        .with("PEPEAUDIO_ENABLE_SITE_EXTRACTORS", "true")
        .with("PEPEAUDIO_ENABLE_CROSS_SERVICE_MATCHING", "true")
        .with("PEPEAUDIO_SPOTIFY_CLIENT_ID", "private-client-id")
        .with("PEPEAUDIO_SPOTIFY_CLIENT_SECRET", "private-client-secret");
    let config = BotRuntimeConfig::from_source(&enabled).expect("complete provider");
    let debug = format!("{config:?}");
    assert!(!debug.contains("private-client-id"));
    assert!(!debug.contains("private-client-secret"));

    let conflict = enabled.with(
        "PEPEAUDIO_SPOTIFY_CLIENT_SECRET_FILE",
        "path-must-not-appear",
    );
    let error = BotRuntimeConfig::from_source(&conflict).expect_err("dual sources");
    assert!(matches!(
        error,
        ConfigError::ConflictingSecretSources { .. }
    ));
    assert!(!format!("{error:?} {error}").contains("path-must-not-appear"));
}

#[test]
fn playlist_and_site_download_limits_are_bounded_by_runtime_capacity() {
    let queue_ten = valid_source().with("PEPEAUDIO_MAX_QUEUE_ITEMS", "10");
    assert_eq!(
        BotRuntimeConfig::from_source(&queue_ten)
            .expect("effective playlist limit")
            .player
            .max_playlist_items
            .get(),
        10
    );

    let too_many = valid_source().with("PEPEAUDIO_MAX_PLAYLIST_ITEMS", "101");
    assert!(BotRuntimeConfig::from_source(&too_many).is_err());

    let oversized_track = valid_source().with("PEPEAUDIO_MAX_SITE_MEDIA_BYTES", "10737418241");
    assert!(matches!(
        BotRuntimeConfig::from_source(&oversized_track),
        Err(ConfigError::Inconsistent { .. })
    ));
}

#[test]
fn api_runtime_does_not_require_bot_token_or_media_tools() {
    let mut source = valid_source();
    source.insert(
        "PEPEAUDIO_DISCORD_CLIENT_SECRET",
        "oauth-secret-long-enough",
    );
    for name in [
        "PEPEAUDIO_DISCORD_TOKEN",
        "PEPEAUDIO_DISCORD_APPLICATION_ID",
        "PEPEAUDIO_SHARD_START",
        "PEPEAUDIO_SHARD_END_EXCLUSIVE",
        "PEPEAUDIO_INSTANCE_ID",
        "PEPEAUDIO_SESSION_COOKIE_SECURE",
        "PEPEAUDIO_SESSION_KEY",
        "PEPEAUDIO_IDLE_DISCONNECT_SECONDS",
        "PEPEAUDIO_DEFAULT_VOLUME_PERCENT",
        "PEPEAUDIO_MAX_QUEUE_ITEMS",
        "PEPEAUDIO_MAX_TRACK_DURATION_SECONDS",
        "PEPEAUDIO_MAX_UPLOAD_BYTES",
        "PEPEAUDIO_MAX_MANAGED_MEDIA_BYTES",
        "PEPEAUDIO_FFMPEG_PATH",
        "PEPEAUDIO_FFPROBE_PATH",
        "PEPEAUDIO_HRIR_DIRECTORY",
        "PEPEAUDIO_UPLOAD_DIRECTORY",
        "PEPEAUDIO_ENABLE_SITE_EXTRACTORS",
        "PEPEAUDIO_YTDLP_PATH",
    ] {
        source.remove(name);
    }
    source.insert("PEPEAUDIO_AUTH_SUCCESS_PATH", "/");
    source.insert("PEPEAUDIO_SESSION_ABSOLUTE_SECONDS", "1800");
    source.insert("PEPEAUDIO_SESSION_IDLE_SECONDS", "1800");
    source.insert("PEPEAUDIO_OAUTH_STATE_SECONDS", "300");

    let config =
        pepeaudio_config::ApiRuntimeConfig::from_source(&source).expect("API-only configuration");
    assert_eq!(config.shard_total.get(), 4);
    assert_eq!(config.auth_success_path, "/");
    assert!(!format!("{config:?}").contains("oauth-secret"));
}

#[test]
fn api_runtime_bounds_the_login_time_membership_window() {
    for (absolute, idle, invalid_name) in [
        ("1801", "1800", "PEPEAUDIO_SESSION_ABSOLUTE_SECONDS"),
        ("900", "901", "PEPEAUDIO_SESSION_IDLE_SECONDS"),
    ] {
        let mut source = valid_source();
        source.insert(
            "PEPEAUDIO_DISCORD_CLIENT_SECRET",
            "oauth-secret-long-enough",
        );
        source.insert("PEPEAUDIO_AUTH_SUCCESS_PATH", "/");
        source.insert("PEPEAUDIO_SESSION_ABSOLUTE_SECONDS", absolute);
        source.insert("PEPEAUDIO_SESSION_IDLE_SECONDS", idle);
        source.insert("PEPEAUDIO_OAUTH_STATE_SECONDS", "300");

        let error = pepeaudio_config::ApiRuntimeConfig::from_source(&source)
            .expect_err("stale membership window must be bounded");
        assert!(error.to_string().contains(invalid_name));
    }
}
