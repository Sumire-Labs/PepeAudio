import { monitorEventLoopDelay } from 'node:perf_hooks';
import type { Client } from 'discord.js';
import * as GuildPlayerManager from '../player/GuildPlayerManager.js';
import { logger } from '../logger.js';

const REPORT_INTERVAL_MS = 60_000;

/**
 * Lightweight operational visibility with zero new dependencies (no
 * prom-client/Grafana - see docs/performance-optimization-plan.md phase 5):
 * one pino log line every 60s with the numbers that matter for spotting
 * trouble at scale, still readable through log aggregation alone even under
 * sharding (each shard logs its own line, tagged by shard - see logger.ts).
 * Starts once the client is ready (guilds.cache is only meaningfully
 * populated after that).
 */
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
          // monitorEventLoopDelay reports nanoseconds; converted to ms for readability.
          eventLoopDelayP99Ms: Math.round(histogram.percentile(99) / 1e6),
          shardId: process.env.SHARDS ?? 'single',
        },
        'Metrics',
      );
      // Reset so each line reflects the last 60s, not an ever-widening
      // all-time percentile that would stop being useful for "is something
      // wrong right now" monitoring.
      histogram.reset();
    }, REPORT_INTERVAL_MS);
    timer.unref();
  });
}
