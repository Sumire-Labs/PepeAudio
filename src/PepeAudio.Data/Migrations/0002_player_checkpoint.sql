CREATE TABLE IF NOT EXISTS guild_player_state (
    guild_id         BIGINT PRIMARY KEY,
    voice_channel_id BIGINT NOT NULL,
    snapshot         JSONB NOT NULL,
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);
