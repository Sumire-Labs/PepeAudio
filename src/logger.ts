import os from 'node:os';
import pino from 'pino';
import { env } from './config/env.js';

export const logger = pino({
  level: env.logLevel,
  // `process.env.SHARDS` is set by ShardingManager (see shard.ts) to this
  // process's own shard id — 'single' when run directly (dist/index.js) or
  // before sharding is introduced, so every log line is traceable to a shard
  // once running under dist/shard.js. Explicit pid/hostname preserves pino's
  // own defaults, which passing a custom `base` otherwise replaces entirely.
  base: { pid: process.pid, hostname: os.hostname(), shard: process.env.SHARDS ?? 'single' },
  // Defense-in-depth: scrub anything secret-looking from log output regardless
  // of which call site produced it. No current code path logs the bot token
  // (verified: discord.js REST errors don't retain the Authorization header),
  // but this makes a future `logger.error({ err })`/`{ config }` that happens to
  // carry credentials fail safe. Matching is by key name; `*` is a single level,
  // so both top-level and one-deep (e.g. under a serialized `err`) are covered.
  redact: {
    paths: [
      'token', '*.token',
      'DISCORD_TOKEN', '*.DISCORD_TOKEN',
      'discordToken', '*.discordToken',
      'authorization', '*.authorization',
      'headers.authorization', '*.headers.authorization',
      'config.headers.authorization', '*.config.headers.authorization',
      // Web dashboard OAuth: never log the client secret, session secret, or any
      // OAuth token/code even if one ends up inside a logged object.
      'client_secret', '*.client_secret',
      'clientSecret', '*.clientSecret',
      'sessionSecret', '*.sessionSecret',
      'access_token', '*.access_token',
      'refresh_token', '*.refresh_token',
      'code', '*.code',
    ],
    censor: '[REDACTED]',
  },
  transport: process.env.NODE_ENV === 'production'
    ? undefined
    : { target: 'pino-pretty', options: { colorize: true, translateTime: 'HH:MM:ss' } },
});

export function childLogger(bindings: Record<string, unknown>) {
  return logger.child(bindings);
}
