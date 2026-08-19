-- Runtime roles must not own this schema and must not run this migration on
-- every replica start. Apply it once with a dedicated migration role/service.
-- Live player state, active queues, sessions, shard leases, and command streams
-- intentionally live outside these tables.

CREATE TABLE hrir_presets (
    preset_id TEXT COLLATE "C" PRIMARY KEY,
    owner_guild_id TEXT COLLATE "C",
    display_name TEXT NOT NULL,
    storage_key TEXT COLLATE "C" NOT NULL UNIQUE,
    sha256_hex CHAR(64) COLLATE "C" NOT NULL,
    sample_rate INTEGER NOT NULL,
    channel_layout TEXT COLLATE "C" NOT NULL,
    file_size_bytes BIGINT NOT NULL,
    license_name TEXT,
    license_url TEXT,
    attribution TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT hrir_presets_id_valid CHECK (
        octet_length(preset_id) BETWEEN 1 AND 128
        AND preset_id = btrim(preset_id)
    ),
    CONSTRAINT hrir_presets_owner_guild_valid CHECK (
        owner_guild_id IS NULL OR (
            owner_guild_id ~ '^[1-9][0-9]{0,19}$'
            AND (
                char_length(owner_guild_id) < 20
                OR owner_guild_id <= '18446744073709551615'
            )
        )
    ),
    CONSTRAINT hrir_presets_display_name_valid CHECK (
        char_length(display_name) BETWEEN 1 AND 120
        AND display_name = btrim(display_name)
    ),
    CONSTRAINT hrir_presets_storage_key_valid CHECK (
        char_length(storage_key) BETWEEN 1 AND 1024
        AND storage_key = btrim(storage_key)
    ),
    CONSTRAINT hrir_presets_checksum_valid CHECK (
        sha256_hex ~ '^[0-9a-f]{64}$'
    ),
    CONSTRAINT hrir_presets_sample_rate_valid CHECK (
        sample_rate IN (44100, 48000)
    ),
    CONSTRAINT hrir_presets_layout_valid CHECK (
        channel_layout IN ('hesuvi_7', 'hesuvi_14')
    ),
    CONSTRAINT hrir_presets_size_valid CHECK (
        file_size_bytes > 0 AND file_size_bytes <= 1073741824
    ),
    CONSTRAINT hrir_presets_license_url_valid CHECK (
        license_url IS NULL OR char_length(license_url) <= 2048
    )
);

CREATE INDEX hrir_presets_owner_display_idx
    ON hrir_presets (owner_guild_id, display_name COLLATE "C");

CREATE TABLE guild_settings (
    guild_id TEXT COLLATE "C" PRIMARY KEY,
    volume_percent SMALLINT NOT NULL DEFAULT 75,
    idle_disconnect_seconds INTEGER NOT NULL DEFAULT 300,
    control_policy TEXT COLLATE "C" NOT NULL DEFAULT 'same_voice_channel',
    dj_role_id TEXT COLLATE "C",
    default_hrir_preset_id TEXT COLLATE "C" REFERENCES hrir_presets(preset_id)
        ON UPDATE CASCADE ON DELETE SET NULL,
    spatial_audio_enabled BOOLEAN NOT NULL DEFAULT false,
    revision BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT guild_settings_id_valid CHECK (
        guild_id ~ '^[1-9][0-9]{0,19}$'
        AND (
            char_length(guild_id) < 20
            OR guild_id <= '18446744073709551615'
        )
    ),
    CONSTRAINT guild_settings_volume_valid CHECK (
        volume_percent BETWEEN 0 AND 100
    ),
    CONSTRAINT guild_settings_idle_valid CHECK (
        idle_disconnect_seconds BETWEEN 30 AND 86400
    ),
    CONSTRAINT guild_settings_policy_valid CHECK (
        control_policy IN ('same_voice_channel', 'dj_only', 'manage_guild')
    ),
    CONSTRAINT guild_settings_dj_role_valid CHECK (
        dj_role_id IS NULL OR (
            dj_role_id ~ '^[1-9][0-9]{0,19}$'
            AND (
                char_length(dj_role_id) < 20
                OR dj_role_id <= '18446744073709551615'
            )
        )
    ),
    CONSTRAINT guild_settings_revision_valid CHECK (revision >= 0)
);

CREATE TABLE playlists (
    playlist_id UUID PRIMARY KEY,
    guild_id TEXT COLLATE "C" NOT NULL,
    owner_user_id TEXT COLLATE "C" NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    visibility TEXT COLLATE "C" NOT NULL DEFAULT 'private',
    revision BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT playlists_guild_valid CHECK (
        guild_id ~ '^[1-9][0-9]{0,19}$'
        AND (
            char_length(guild_id) < 20
            OR guild_id <= '18446744073709551615'
        )
    ),
    CONSTRAINT playlists_owner_valid CHECK (
        owner_user_id ~ '^[1-9][0-9]{0,19}$'
        AND (
            char_length(owner_user_id) < 20
            OR owner_user_id <= '18446744073709551615'
        )
    ),
    CONSTRAINT playlists_name_valid CHECK (
        char_length(name) BETWEEN 1 AND 100 AND name = btrim(name)
    ),
    CONSTRAINT playlists_description_valid CHECK (
        description IS NULL OR char_length(description) <= 2000
    ),
    CONSTRAINT playlists_visibility_valid CHECK (
        visibility IN ('private', 'guild')
    ),
    CONSTRAINT playlists_revision_valid CHECK (revision >= 0)
);

CREATE INDEX playlists_guild_updated_idx
    ON playlists (guild_id, updated_at DESC);
CREATE INDEX playlists_owner_updated_idx
    ON playlists (owner_user_id, updated_at DESC);

CREATE TABLE playlist_tracks (
    track_id UUID PRIMARY KEY,
    playlist_id UUID NOT NULL REFERENCES playlists(playlist_id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    source_kind TEXT COLLATE "C" NOT NULL,
    source_reference TEXT NOT NULL,
    title TEXT NOT NULL,
    duration_ms BIGINT,
    added_by_user_id TEXT COLLATE "C" NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT playlist_tracks_position_valid CHECK (position >= 0),
    CONSTRAINT playlist_tracks_source_kind_valid CHECK (
        source_kind IN ('direct_url', 'managed_upload')
    ),
    CONSTRAINT playlist_tracks_source_reference_valid CHECK (
        char_length(source_reference) BETWEEN 1 AND 4096
    ),
    CONSTRAINT playlist_tracks_title_valid CHECK (
        char_length(title) BETWEEN 1 AND 512
    ),
    CONSTRAINT playlist_tracks_duration_valid CHECK (
        duration_ms IS NULL OR duration_ms BETWEEN 0 AND 604800000
    ),
    CONSTRAINT playlist_tracks_added_by_valid CHECK (
        added_by_user_id ~ '^[1-9][0-9]{0,19}$'
        AND (
            char_length(added_by_user_id) < 20
            OR added_by_user_id <= '18446744073709551615'
        )
    ),
    CONSTRAINT playlist_tracks_position_unique UNIQUE (playlist_id, position)
);

CREATE INDEX playlist_tracks_playlist_order_idx
    ON playlist_tracks (playlist_id, position);
