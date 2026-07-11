import type { Client } from 'discord.js';
import { logger } from '../logger.js';

/**
 * `onFatal` is invoked for conditions Node considers unrecoverable. Per Node's
 * own guidance, after an 'uncaughtException' the process is in an undefined
 * state — continuing to run (as this used to, logging and carrying on) risks
 * leaked voice connections/ffmpeg children and corrupted in-memory state. We
 * instead hand off to a graceful shutdown that exits non-zero so a supervisor
 * (Docker restart policy / orchestrator) brings up a clean process.
 */
export function registerErrorEvents(
  client: Client,
  onFatal: (reason: string, err: unknown) => void,
): void {
  client.on('error', (err) => logger.error({ err }, 'Discord client error'));
  client.on('shardError', (err) => logger.error({ err }, 'Discord shard error'));

  // Unhandled rejections are frequently benign in the discord.js/voice stack
  // (a fire-and-forget REST call that rejected). Log loudly, but don't treat as
  // fatal — turning every recoverable hiccup into a restart would be worse.
  process.on('unhandledRejection', (reason) => {
    logger.error({ err: reason }, 'Unhandled promise rejection');
  });

  process.on('uncaughtException', (err) => {
    logger.error({ err }, 'Uncaught exception - treating as fatal and shutting down');
    onFatal('uncaughtException', err);
  });
}
