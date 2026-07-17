import { ActivityType, type Client } from 'discord.js';
import { logger } from '../logger.js';

const UPDATE_INTERVAL_MS = 60_000;

/**
 * Advertises the process's resident-set memory (RSS) as the bot's custom
 * status - e.g. "🧠 128 MB" - refreshed every 60s. RSS is the same figure the
 * metrics log line reports (see metricsReporter.ts): the whole PROCESS's
 * memory, not any single guild's, so under sharding each shard's presence
 * reflects its own process's RSS.
 *
 * 60s sits comfortably under Discord's presence-update rate limit (5 updates
 * per 20s per gateway session) and memory doesn't move fast enough to justify
 * anything tighter.
 */
export function startPresenceReporter(client: Client): void {
  client.once('clientReady', (readyClient) => {
    const update = (): void => {
      const rssMb = Math.round(process.memoryUsage().rss / 1024 / 1024);
      try {
        readyClient.user.setActivity({
          // `name` is required by the gateway but is NOT what renders for a
          // Custom activity - the client displays `state`. Keeping name stable
          // avoids it leaking into any surface that does read it.
          name: 'memory',
          type: ActivityType.Custom,
          state: `🧠 ${rssMb} MB`,
        });
      } catch (err) {
        // setActivity only queues a gateway op, but guard anyway: an uncaught
        // throw inside a setInterval callback would surface as an
        // uncaughtException and trip the fatal-error shutdown handler.
        logger.warn({ err }, 'Failed to update presence with memory usage');
      }
    };

    update(); // set immediately rather than leave the bot statusless for the first interval
    const timer = setInterval(update, UPDATE_INTERVAL_MS);
    timer.unref();
  });
}
