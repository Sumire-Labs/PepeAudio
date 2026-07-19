import type { Client } from 'discord.js';
import { logger } from '../logger.js';

// After 'uncaughtException' Node's state is undefined; continuing risks leaked
// voice connections/ffmpeg children, so hand off to graceful shutdown (exit
// non-zero) and let a supervisor restart a clean process.
export function registerErrorEvents(
  client: Client,
  onFatal: (reason: string, err: unknown) => void,
): void {
  client.on('error', (err) => logger.error({ err }, 'Discord client error'));
  client.on('shardError', (err) => logger.error({ err }, 'Discord shard error'));

  // Rejections are usually benign in discord.js/voice (fire-and-forget REST):
  // log, but don't treat as fatal or every hiccup becomes a restart.
  process.on('unhandledRejection', (reason) => {
    logger.error({ err: reason }, 'Unhandled promise rejection');
  });

  process.on('uncaughtException', (err) => {
    logger.error({ err }, 'Uncaught exception - treating as fatal and shutting down');
    onFatal('uncaughtException', err);
  });
}
