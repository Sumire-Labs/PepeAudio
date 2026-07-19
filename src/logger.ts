import os from 'node:os';
import pino from 'pino';
import { env } from './config/env.js';

export const logger = pino({
  level: env.logLevel,
  // SHARDS is set per-process by ShardingManager (shard.ts); 'single' when unsharded.
  // pid/hostname must be listed explicitly — a custom `base` replaces pino's defaults entirely.
  base: { pid: process.pid, hostname: os.hostname(), shard: process.env.SHARDS ?? 'single' },
  // Security: fail-safe redaction of secret-looking keys regardless of call site.
  // `*` matches a single level, so paths cover both top-level and one-deep (e.g. under `err`).
  redact: {
    paths: [
      'token', '*.token',
      'DISCORD_TOKEN', '*.DISCORD_TOKEN',
      'discordToken', '*.discordToken',
      'authorization', '*.authorization',
      'headers.authorization', '*.headers.authorization',
      'config.headers.authorization', '*.config.headers.authorization',
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
