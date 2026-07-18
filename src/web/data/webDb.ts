/**
 * A separate SQLite database (pepeaudio-web.sqlite) for web-dashboard data
 * (saved playlists). Kept apart from the bot's pepeaudio.sqlite on purpose:
 * under sharding every shard process writes the main DB, and the web server runs
 * in a different process (the ShardingManager). Giving the web server its own DB
 * file makes it the only writer here — no multi-process contention. This module
 * is imported only under the dashboard-enabled path, so it's a no-op otherwise.
 */
import path from 'node:path';
import { mkdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import Database from 'better-sqlite3';
import { env } from '../../config/env.js';
import { logger } from '../../logger.js';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
// dist/web/data/webDb.js → repo root is three levels up (mirrors data/db.ts's two).
const PROJECT_ROOT = path.resolve(__dirname, '..', '..', '..');
const DATA_DIR = env.dataDir ?? PROJECT_ROOT;
mkdirSync(DATA_DIR, { recursive: true });
const DB_PATH = path.join(DATA_DIR, 'pepeaudio-web.sqlite');

export const webDb = new Database(DB_PATH);
webDb.pragma('journal_mode = WAL');
// Single writer, but set a busy_timeout anyway so a WAL checkpoint never surfaces
// a transient SQLITE_BUSY to a request handler.
webDb.pragma('busy_timeout = 5000');

webDb.exec(`
  CREATE TABLE IF NOT EXISTS web_playlists (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    name TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
  );
  CREATE INDEX IF NOT EXISTS idx_web_playlists_user ON web_playlists(user_id);
  CREATE TABLE IF NOT EXISTS web_playlist_tracks (
    playlist_id TEXT NOT NULL,
    position INTEGER NOT NULL,
    source_url TEXT NOT NULL,
    title TEXT NOT NULL,
    artist TEXT NOT NULL,
    thumbnail_url TEXT,
    source_type TEXT NOT NULL,
    duration_ms INTEGER,
    PRIMARY KEY (playlist_id, position)
  );
`);

logger.info({ dbPath: DB_PATH }, 'Web dashboard SQLite database ready');
