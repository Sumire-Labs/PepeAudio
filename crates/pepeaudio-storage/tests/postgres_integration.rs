//! Optional live `PostgreSQL` migration and repository smoke test.
//!
//! Set `PEPEAUDIO_TEST_DATABASE_URL` to a disposable database and run this
//! ignored test explicitly. It applies the embedded migrations.

use std::time::Duration;

use pepeaudio_core::{GuildId, HrirPresetId, Volume};
use pepeaudio_storage::{
    ControlPolicy, GuildSettings, GuildSettingsRepository, HrirChannelLayout, HrirPresetMetadata,
    HrirPresetRepository, PostgresStorage, SettingsRevision,
};
use sqlx::migrate::Migrator;
use time::OffsetDateTime;

static MIGRATOR: Migrator = sqlx::migrate!("../../migrations");

#[tokio::test]
#[ignore = "requires a disposable PostgreSQL database"]
async fn migrates_and_round_trips_versioned_guild_settings() {
    let url = std::env::var("PEPEAUDIO_TEST_DATABASE_URL")
        .expect("set PEPEAUDIO_TEST_DATABASE_URL for the ignored test");
    let storage = PostgresStorage::connect(&url, 1)
        .await
        .expect("connect to PostgreSQL");
    MIGRATOR
        .run(storage.pool())
        .await
        .expect("apply migrations");
    assert_connection_timeouts(&storage).await;

    let guild_id =
        GuildId::new(u64::MAX - u64::from(std::process::id())).expect("unique test guild");
    sqlx::query("DELETE FROM guild_settings WHERE guild_id = $1")
        .bind(guild_id.to_string())
        .execute(storage.pool())
        .await
        .expect("remove stale test row");

    let defaults = GuildSettings {
        guild_id,
        volume: Volume::DEFAULT,
        idle_disconnect: Duration::from_mins(5),
        control_policy: ControlPolicy::SameVoiceChannel,
        dj_role_id: None,
        default_hrir_preset_id: None,
        spatial_audio_enabled: false,
        revision: SettingsRevision::INITIAL,
        created_at: OffsetDateTime::UNIX_EPOCH,
        updated_at: OffsetDateTime::UNIX_EPOCH,
    };
    let created = storage
        .create_guild_settings(&defaults)
        .await
        .expect("create settings");
    assert_eq!(created.volume, Volume::DEFAULT);

    let mut changed = created.clone();
    changed.volume = Volume::new(50).expect("valid volume");
    let updated = storage
        .update_guild_settings(&changed, SettingsRevision::INITIAL)
        .await
        .expect("update settings")
        .expect("matching revision");
    assert_eq!(updated.volume.percent(), 50);
    assert_eq!(updated.revision, SettingsRevision::new(1));

    synchronizes_global_hrir_catalog(&storage, guild_id).await;

    sqlx::query("DELETE FROM guild_settings WHERE guild_id = $1")
        .bind(guild_id.to_string())
        .execute(storage.pool())
        .await
        .expect("clean test row");
    storage.close().await;
}

async fn assert_connection_timeouts(storage: &PostgresStorage) {
    let statement_timeout: String = sqlx::query_scalar("SHOW statement_timeout")
        .fetch_one(storage.pool())
        .await
        .expect("read statement timeout");
    let lock_timeout: String = sqlx::query_scalar("SHOW lock_timeout")
        .fetch_one(storage.pool())
        .await
        .expect("read lock timeout");
    let idle_transaction_timeout: String =
        sqlx::query_scalar("SHOW idle_in_transaction_session_timeout")
            .fetch_one(storage.pool())
            .await
            .expect("read idle transaction timeout");

    assert_eq!(statement_timeout, "10s");
    assert_eq!(lock_timeout, "5s");
    assert_eq!(idle_transaction_timeout, "15s");
}

async fn synchronizes_global_hrir_catalog(storage: &PostgresStorage, guild_id: GuildId) {
    let suffix = std::process::id();
    let first_id = HrirPresetId::new(format!("storage-smoke-{suffix}-a")).expect("preset ID");
    let second_id = HrirPresetId::new(format!("storage-smoke-{suffix}-b")).expect("preset ID");
    let preset = |id: HrirPresetId, marker: char| HrirPresetMetadata {
        preset_id: id,
        owner_guild_id: None,
        display_name: format!("Storage smoke {marker}"),
        storage_key: format!("storage-smoke-{suffix}-{marker}.wav"),
        sha256_hex: marker.to_string().repeat(64),
        sample_rate: 48_000,
        channel_layout: HrirChannelLayout::Hesuvi14,
        file_size_bytes: 1024,
        license_name: None,
        license_url: None,
        attribution: None,
        created_at: OffsetDateTime::UNIX_EPOCH,
    };
    let first = preset(first_id.clone(), 'a');
    let second = preset(second_id.clone(), 'b');
    storage
        .synchronize_global_hrir_presets(&[first.clone(), second])
        .await
        .expect("install global catalog");
    storage
        .synchronize_global_hrir_presets(&[first])
        .await
        .expect("remove stale global preset");
    let listed = storage
        .list_hrir_presets(guild_id)
        .await
        .expect("list synchronized presets");
    assert!(listed.iter().any(|preset| preset.preset_id == first_id));
    assert!(!listed.iter().any(|preset| preset.preset_id == second_id));
    sqlx::query("DELETE FROM hrir_presets WHERE preset_id = $1")
        .bind(first_id.as_str())
        .execute(storage.pool())
        .await
        .expect("clean synchronized preset");
}
