import { ActivityType, type Client } from 'discord.js';
import { logger } from '../logger.js';

const UPDATE_INTERVAL_MS = 60_000;

// 60s interval stays under Discord's presence-update rate limit (5 per 20s per gateway session).
export function startPresenceReporter(client: Client): void {
  client.once('clientReady', (readyClient) => {
    const update = (): void => {
      const rssMb = Math.round(process.memoryUsage().rss / 1024 / 1024);
      try {
        readyClient.user.setActivity({
          // Gateway requires `name`, but a Custom activity renders `state`, not `name`.
          name: 'memory',
          type: ActivityType.Custom,
          state: `🧠 ${rssMb} MB`,
        });
      } catch (err) {
        // Guard: an uncaught throw in this setInterval callback would trip the fatal-error shutdown handler.
        logger.warn({ err }, 'Failed to update presence with memory usage');
      }
    };

    update();
    const timer = setInterval(update, UPDATE_INTERVAL_MS);
    timer.unref();
  });
}
