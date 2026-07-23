-- Per-user web dashboard playlists. user_id is the Discord user id (from the JWT 'sub' claim).
-- Tracks are a dense, position-ordered list; source_type is the SourceKind enum ordinal.
CREATE TABLE IF NOT EXISTS web_playlists (
    id          UUID PRIMARY KEY,
    user_id     BIGINT NOT NULL,
    name        TEXT NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_web_playlists_user ON web_playlists(user_id, updated_at DESC);

CREATE TABLE IF NOT EXISTS web_playlist_tracks (
    playlist_id   UUID NOT NULL REFERENCES web_playlists(id) ON DELETE CASCADE,
    position      INTEGER NOT NULL,
    source_url    TEXT NOT NULL,
    title         TEXT NOT NULL,
    artist        TEXT NOT NULL,
    thumbnail_url TEXT NULL,
    source_type   SMALLINT NOT NULL,
    duration_ms   BIGINT NULL,
    PRIMARY KEY (playlist_id, position)
);
