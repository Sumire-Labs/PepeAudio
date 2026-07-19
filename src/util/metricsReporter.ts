import { monitorEventLoopDelay } from 'node:perf_hooks';
import type { Client } from 'discord.js';
import * as GuildPlayerManager from '../player/GuildPlayerManager.js';
import { logger } from '../logger.js';

const REPORT_INTERVAL_MS = 60_000;

// Started on clientReady: guilds.cache is only meaningfully populated after that.
export function startMetricsReporter(client: Client): void {
  client.once('clientReady', () => {
    const histogram = monitorEventLoopDelay();
    histogram.enable();

    const timer = setInterval(() => {
      const players = GuildPlayerManager.all();
      const activePlayers = players.filter((p) => p.status === 'playing').length;
      logger.info(
        {
          activePlayers,
          totalPlayers: players.length,
          guilds: client.guilds.cache.size,
          rssMb: Math.round(process.memoryUsage().rss / 1024 / 1024),
          // monitorEventLoopDelay reports nanoseconds; convert to ms.
          eventLoopDelayP99Ms: Math.round(histogram.percentile(99) / 1e6),
          shardId: process.env.SHARDS ?? 'single',
        },
        'Metrics',
      );
      // Reset so each line's percentile reflects the last 60s, not all-time.
      histogram.reset();
    }, REPORT_INTERVAL_MS);
    timer.unref();
  });
}
