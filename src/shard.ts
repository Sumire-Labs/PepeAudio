import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { ShardingManager, type Shard } from 'discord.js';
import { env } from './config/env.js';
import { logger } from './logger.js';
import { loadWebEnv, resolveClientDir } from './web/config.js';
import type { WebServerHandle } from './web/index.js';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const CLIENT_ENTRY = path.join(__dirname, 'index.js');

const manager = new ShardingManager(CLIENT_ENTRY, {
  token: env.discordToken,
  totalShards: 'auto',
  respawn: true,
  mode: 'process',
});

manager.on('shardCreate', (shard: Shard) => {
  logger.info({ shardId: shard.id }, 'Shard created');
  shard.on('spawn', () => logger.info({ shardId: shard.id, pid: shard.process?.pid }, 'Shard process spawned'));
  shard.on('death', () => logger.warn({ shardId: shard.id }, 'Shard process died'));
  shard.on('disconnect', () => logger.warn({ shardId: shard.id }, 'Shard disconnected from the gateway'));
  shard.on('error', (err) => logger.error({ err, shardId: shard.id }, 'Shard error'));
});

const SHUTDOWN_TIMEOUT_MS = 8_000;

let webServerHandle: WebServerHandle | undefined;

let shuttingDown = false;
async function shutdown(signal: string): Promise<void> {
  if (shuttingDown) return;
  shuttingDown = true;
  logger.info({ signal }, 'ShardingManager: forwarding shutdown to all shards');

  // Manager must forward signals itself — it's PID 1 here, not any shard.
  // Avoid Shard#kill(): it fires 'death' synchronously before the OS process
  // actually exits, so we can't observe real graceful shutdown. Killing
  // shard.process directly leaves discord.js's own exit listener in place, so
  // respawn must be disabled first to stop it respawning the shard on exit.
  manager.respawn = false;

  // Stop accepting web requests before tearing the shards down.
  await webServerHandle?.close();

  const exits = [...manager.shards.values()].map(
    (shard) =>
      new Promise<void>((resolve) => {
        const proc = shard.process;
        if (!proc || proc.exitCode !== null || proc.signalCode !== null) {
          resolve();
          return;
        }
        proc.once('exit', () => resolve());
        // The child's SIGTERM handler exits within its own 5s watchdog, well
        // inside this manager's 8s budget.
        proc.kill('SIGTERM');
      }),
  );

  await Promise.race([
    Promise.all(exits),
    new Promise<void>((resolve) => setTimeout(resolve, SHUTDOWN_TIMEOUT_MS)),
  ]);

  for (const shard of manager.shards.values()) {
    const proc = shard.process;
    if (proc && proc.exitCode === null && proc.signalCode === null) {
      logger.warn({ shardId: shard.id }, 'Shard did not exit within the timeout - sending SIGKILL');
      proc.kill('SIGKILL');
    }
  }

  process.exit(0);
}

// On Windows, process.kill('SIGTERM') terminates immediately without invoking
// the target's handler, so shards won't shut down gracefully there (prod runs
// Linux containers).
process.on('SIGINT', () => void shutdown('SIGINT'));
process.on('SIGTERM', () => void shutdown('SIGTERM'));

await manager.spawn();
logger.info({ totalShards: manager.totalShards }, 'ShardingManager: all shards spawned');

// Web dashboard (opt-in). Must start after spawn so manager.totalShards is known
// and shards are alive.
const webEnv = loadWebEnv(env.clientId, resolveClientDir(path.dirname(fileURLToPath(import.meta.url))));
if (webEnv) {
  const { ShardedBridge } = await import('./web/bridge/sharded.js');
  const { startWebServer } = await import('./web/index.js');
  webServerHandle = startWebServer({ bridge: new ShardedBridge(manager), env: webEnv });
}
