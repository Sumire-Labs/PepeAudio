import 'dotenv/config';
import { readFileSync } from 'node:fs';

export function required(name: string): string {
  const value = process.env[name];
  if (!value) {
    throw new Error(`Missing required environment variable: ${name}`);
  }
  return value;
}

/**
 * Reads a secret from `NAME`, or from a file at `NAME_FILE`. The file indirection
 * lets Docker/K8s file-mounted secrets supply the value without it living in an
 * env var (which leaks via `docker inspect`, /proc/environ, child-proc inheritance).
 */
export function requiredSecret(name: string): string {
  const filePath = process.env[`${name}_FILE`];
  if (filePath) {
    let contents: string;
    try {
      contents = readFileSync(filePath, 'utf8');
    } catch (err) {
      throw new Error(`Failed to read ${name}_FILE (${filePath}): ${(err as Error).message}`, { cause: err });
    }
    const value = contents.trim();
    if (!value) {
      throw new Error(`Secret file ${name}_FILE (${filePath}) is empty`);
    }
    return value;
  }
  return required(name);
}

export const env = {
  discordToken: requiredSecret('DISCORD_TOKEN'),
  clientId: required('CLIENT_ID'), // public application id, not a secret
  guildId: process.env.GUILD_ID || null,
  logLevel: process.env.LOG_LEVEL || 'info',
  ffmpegPathOverride: process.env.FFMPEG_PATH || null,
  hrirProfilesDirOverride: process.env.HRIR_PROFILES_DIR || null,
  /** Directory for the SQLite DB (see data/db.ts). Unset = project root. */
  dataDir: process.env.DATA_DIR || null,
};
