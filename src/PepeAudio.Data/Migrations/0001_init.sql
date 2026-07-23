CREATE TABLE IF NOT EXISTS guilds (
    guild_id   BIGINT PRIMARY KEY,
    name       TEXT,
    joined_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    left_at    TIMESTAMPTZ NULL
);

CREATE TABLE IF NOT EXISTS guild_settings (
    guild_id              BIGINT PRIMARY KEY REFERENCES guilds(guild_id),
    aura_enabled          BOOLEAN NOT NULL DEFAULT true,
    preset_name           TEXT NOT NULL DEFAULT 'Aura',
    volume                SMALLINT NOT NULL DEFAULT 10 CHECK (volume BETWEEN 0 AND 200),
    normalization         TEXT NOT NULL DEFAULT 'Off',
    crossfade_ms          SMALLINT NOT NULL DEFAULT 0 CHECK (crossfade_ms BETWEEN 0 AND 12000),
    effect_chain          JSONB NULL,
    dj_role_id            BIGINT NULL,
    autoplay              BOOLEAN NOT NULL DEFAULT false,
    bound_text_channel_id BIGINT NULL,
    locale                TEXT NOT NULL DEFAULT 'en-US',
    updated_at            TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS track_cache (
    id          BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    cache_key   TEXT NOT NULL UNIQUE,
    source      TEXT NOT NULL,
    source_id   TEXT,
    isrc        TEXT,
    title       TEXT,
    author      TEXT,
    duration_ms INTEGER,
    thumbnail_url TEXT,
    is_live     BOOLEAN NOT NULL DEFAULT false,
    match_score REAL,
    resolved_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    refreshed_at TIMESTAMPTZ,
    expires_at  TIMESTAMPTZ NULL
);
CREATE INDEX IF NOT EXISTS idx_track_cache_isrc ON track_cache(isrc);
CREATE INDEX IF NOT EXISTS idx_track_cache_source ON track_cache(source, source_id);
