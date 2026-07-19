import path from 'node:path';
import { mkdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import Database from 'better-sqlite3';
import { env } from '../config/env.js';
import { logger } from '../logger.js';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const PROJECT_ROOT = path.resolve(__dirname, '..', '..');
// DATA_DIR keeps the SQLite file outside the code tree (e.g. a Docker volume); unset = project root.
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

// ALTER TABLE ADD COLUMN throws if the column exists, so guard each with the PRAGMA check.
// Every column added after the initial CREATE must be migrated here too: guildSettingsRepo
// prepares statements at import time, so a missing column throws "no such column" at startup.
const guildSettingsColumns = db.prepare('PRAGMA table_info(guild_settings)').all() as Array<{ name: string }>;
const existingColumnNames = new Set(guildSettingsColumns.map((col) => col.name));
const guildSettingsMigrations: Array<{ column: string; ddl: string }> = [
  { column: 'default_hrir_profile', ddl: 'ALTER TABLE guild_settings ADD COLUMN default_hrir_profile TEXT' },
  { column: 'stay_247', ddl: 'ALTER TABLE guild_settings ADD COLUMN stay_247 INTEGER NOT NULL DEFAULT 0' },
  { column: 'autoplay', ddl: 'ALTER TABLE guild_settings ADD COLUMN autoplay INTEGER NOT NULL DEFAULT 0' },
  { column: 'default_enhancer_mode', ddl: "ALTER TABLE guild_settings ADD COLUMN default_enhancer_mode TEXT NOT NULL DEFAULT 'off'" },
];
for (const { column, ddl } of guildSettingsMigrations) {
  if (!existingColumnNames.has(column)) {
    db.exec(ddl);
  }
}

logger.info({ dbPath: DB_PATH }, 'SQLite database ready');
