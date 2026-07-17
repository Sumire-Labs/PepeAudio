import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { ShardingManager, type Shard } from 'discord.js';
import { env } from './config/env.js';
import { logger } from './logger.js';
import { loadWebEnv, resolveClientDir } from './web/config.js';
import type { WebServerHandle } from './web/index.js';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
// shard.js and index.js are built side by side into dist/ by `pnpm build`.
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

  // ShardingManager does not forward signals to its children on its own —
  // PID 1 here is this manager process, not any shard's Client process.
  //
  // Deliberately NOT using Shard#kill(): it removes its own 'exit' listener
  // and fires its internal bookkeeping (the 'death' event) synchronously,
  // BEFORE the real OS process has actually exited — so it gives no way to
  // observe when a shard has genuinely finished its own graceful shutdown.
  // Killing shard.process directly leaves discord.js's own exit listener
  // (attached in Shard#spawn) in place, so disabling manager.respawn first is
  // what prevents that listener from respawning the shard once it exits.
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
        // The child's own SIGTERM handler (index.ts's shutdown()) runs its
        // graceful teardown (stop every player: VC disconnect, ffmpeg
        // teardown, flush pending settings) with its own 5s watchdog, so it
        // will always exit well within this manager's 8s budget.
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

// Windows note: process.kill('SIGTERM') on Windows terminates immediately
// rather than invoking a handler in the target process, so a shard started
// via `node dist/shard.js` on Windows will not shut down gracefully — this is
// expected/known, not a bug, per the plan (production runs Linux containers).
process.on('SIGINT', () => void shutdown('SIGINT'));
process.on('SIGTERM', () => void shutdown('SIGTERM'));

await manager.spawn();
logger.info({ totalShards: manager.totalShards }, 'ShardingManager: all shards spawned');

// Web dashboard (opt-in). Started AFTER spawn so manager.totalShards is known and
// shards are alive. Runs in this manager process; reaches shards via broadcastEval.
const webEnv = loadWebEnv(env.clientId, resolveClientDir(path.dirname(fileURLToPath(import.meta.url))));
if (webEnv) {
  const { ShardedBridge } = await import('./web/bridge/sharded.js');
  const { startWebServer } = await import('./web/index.js');
  webServerHandle = startWebServer({ bridge: new ShardedBridge(manager), env: webEnv });
}
