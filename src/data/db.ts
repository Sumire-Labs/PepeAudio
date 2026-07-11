import path from 'node:path';
import { mkdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import Database from 'better-sqlite3';
import { env } from '../config/env.js';
import { logger } from '../logger.js';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const PROJECT_ROOT = path.resolve(__dirname, '..', '..');
// DATA_DIR lets the SQLite file live outside the code tree (e.g. a mounted
// Docker volume) so guild settings survive container/image recreation and a
// non-root runtime can own a writable location. Unset = project root, which
// preserves the original behavior for local runs.
const DATA_DIR = env.dataDir ?? PROJECT_ROOT;
mkdirSync(DATA_DIR, { recursive: true });
const DB_PATH = path.join(DATA_DIR, 'pepeaudio.sqlite');

export const db = new Database(DB_PATH);
db.pragma('journal_mode = WAL');

db.exec(`
  CREATE TABLE IF NOT EXISTS guild_settings (
    guild_id TEXT PRIMARY KEY,
    default_volume INTEGER NOT NULL DEFAULT 100,
    default_spatial_mode TEXT NOT NULL DEFAULT 'off',
    dj_role_id TEXT,
    permission_mode TEXT NOT NULL DEFAULT 'same-voice-channel',
    updated_at INTEGER NOT NULL
  );
`);

// Lightweight migrations: ALTER TABLE ADD COLUMN errors if the column already
// exists, so guard each with a PRAGMA check rather than a bare try/catch —
// this runs on every startup against a table that may already exist from
// before any of these columns were introduced. CREATE TABLE IF NOT EXISTS
// above only defines the columns present since the very first release; every
// column added later must be migrated in here too, or a guild_settings table
// that predates it will be missing the column and every read/write against it
// (guildSettingsRepo.ts's prepared statements, evaluated at import time) will
// throw "no such column" and crash the whole process at startup.
const guildSettingsColumns = db.prepare('PRAGMA table_info(guild_settings)').all() as Array<{ name: string }>;
const existingColumnNames = new Set(guildSettingsColumns.map((col) => col.name));
const guildSettingsMigrations: Array<{ column: string; ddl: string }> = [
  { column: 'default_hrir_profile', ddl: 'ALTER TABLE guild_settings ADD COLUMN default_hrir_profile TEXT' },
  { column: 'stay_247', ddl: 'ALTER TABLE guild_settings ADD COLUMN stay_247 INTEGER NOT NULL DEFAULT 0' },
  { column: 'autoplay', ddl: 'ALTER TABLE guild_settings ADD COLUMN autoplay INTEGER NOT NULL DEFAULT 0' },
];
for (const { column, ddl } of guildSettingsMigrations) {
  if (!existingColumnNames.has(column)) {
    db.exec(ddl);
  }
}

logger.info({ dbPath: DB_PATH }, 'SQLite database ready');
