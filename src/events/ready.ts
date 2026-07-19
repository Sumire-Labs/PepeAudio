import type { Client } from 'discord.js';
import { commands } from '../commands/index.js';
import { env } from '../config/env.js';
import { logger } from '../logger.js';

const REGISTRATION_RETRY_DELAYS_MS = [5_000, 15_000, 60_000];

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/**
 * Retries with backoff — a transient Discord REST hiccup at boot otherwise
 * leaves the bot "ready" with zero slash commands registered until a manual
 * restart. Global registration (no GUILD_ID) can take ~1h to propagate.
 */
export function registerReadyEvent(client: Client): void {
  client.once('clientReady', async (readyClient) => {
    logger.info({ tag: readyClient.user.tag, guilds: readyClient.guilds.cache.size }, 'Bot is ready');

    const body = commands.map((cmd) => cmd.data.toJSON());

    for (let attempt = 0; ; attempt++) {
      try {
        if (env.guildId) {
          const guild = await readyClient.guilds.fetch(env.guildId);
          await guild.commands.set(body);
          logger.info({ guildId: env.guildId, count: body.length }, 'Slash commands registered to guild (instant)');
        } else {
          await readyClient.application.commands.set(body);
          logger.info({ count: body.length }, 'Slash commands registered globally (may take up to 1h to propagate)');
        }
        return;
      } catch (err) {
        const nextDelay = REGISTRATION_RETRY_DELAYS_MS[attempt];
        if (nextDelay === undefined) {
          logger.error({ err, attempts: attempt + 1 }, 'Failed to auto-register slash commands after all retries - giving up');
          return;
        }
        logger.warn({ err, attempt: attempt + 1, nextDelayMs: nextDelay }, 'Slash command registration failed - retrying');
        await delay(nextDelay);
      }
    }
  });
}
