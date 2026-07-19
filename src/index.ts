import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { Client, GatewayIntentBits, Options, Partials } from 'discord.js';
import { generateDependencyReport } from '@discordjs/voice';
import { env } from './config/env.js';
import { loadWebEnv, resolveClientDir } from './web/config.js';
import type { WebServerHandle } from './web/index.js';
import type { ShardBridgeHandle } from './web/shardBridgeGlobal.js';
import { initFfmpeg } from './config/ffmpegResolver.js';
import { setFfmpegCapabilities } from './config/ffmpegState.js';
import { initHrirProfiles } from './config/hrirProfilesState.js';
import { initPanelManager } from './ui/panelManager.js';
import { registerReadyEvent } from './events/ready.js';
import { registerInteractionCreateEvent } from './events/interactionCreate.js';
import { registerVoiceStateUpdateEvent } from './events/voiceStateUpdate.js';
import { registerErrorEvents } from './events/errorEvents.js';
import { startMetricsReporter } from './util/metricsReporter.js';
import { startPresenceReporter } from './util/presenceReporter.js';
import * as GuildPlayerManager from './player/GuildPlayerManager.js';
import { sweepStaleTrackBuffers } from './player/trackBufferSweep.js';
import { logger } from './logger.js';
import './data/db.js'; // ensures the SQLite schema migration runs before anything else touches it

const ffmpegCapabilities = initFfmpeg();
setFfmpegCapabilities(ffmpegCapabilities);
initPanelManager(ffmpegCapabilities);
initHrirProfiles(ffmpegCapabilities.path, env.hrirProfilesDirOverride);

// Fire-and-forget (never rejects) so reclaiming orphaned reseek temp files can't delay login.
void sweepStaleTrackBuffers();

// The report lists every package whether present or not, so detection must
// match the literal "- <name>: not found" line, not just the package name.
const dependencyReport = generateDependencyReport();
logger.info({ dependencyReport }, 'Voice dependency report');
if (dependencyReport.includes('- @discordjs/opus: not found')) {
  logger.warn('Native @discordjs/opus not available - falling back to the pure-JS opusscript encoder, which is significantly slower per stream at scale');
}
if (dependencyReport.includes('- sodium-native: not found')) {
  logger.warn('Native sodium-native not available - falling back to a pure-JS/WASM encryption library, which is slower at scale');
}

const client = new Client({
  intents: [GatewayIntentBits.Guilds, GatewayIntentBits.GuildVoiceStates],
  partials: [Partials.Channel],
  // Security: track titles come from external metadata and can contain mention
  // tokens (@everyone, <@&roleId>). escapeMd neutralizes markdown but not
  // mentions, so parse:[] is required to suppress crafted-title pings.
  allowedMentions: { parse: [] },
  makeCache: Options.cacheWithLimits({
    MessageManager: 0, // panelManager keeps its own Message refs (never re-fetched via this cache), so 0 is safe.
    UserManager: 100,
    // GuildMemberManager left UNLIMITED: the alone-timeout check reads
    // channel.members (from this cache); a limit could silently break it.
  }),
});

registerReadyEvent(client);
registerInteractionCreateEvent(client);
registerVoiceStateUpdateEvent(client);
startMetricsReporter(client);
startPresenceReporter(client);
registerErrorEvents(client, (reason, err) => {
  logger.error({ err, reason }, 'Fatal error - shutting down so the process can restart clean');
  void shutdown(reason, 1);
});

let webServerHandle: WebServerHandle | undefined;
let shardBridgeHandle: ShardBridgeHandle | undefined;

let shuttingDown = false;
async function shutdown(reason: string, exitCode = 0): Promise<void> {
  if (shuttingDown) return;
  shuttingDown = true;
  logger.info({ reason, exitCode }, 'Shutting down');
  // Watchdog: force-exit if graceful shutdown hangs (e.g. a stuck voice connection) so the process still restarts.
  const watchdog = setTimeout(() => {
    logger.error('Graceful shutdown timed out - forcing exit');
    process.exit(exitCode === 0 ? 1 : exitCode);
  }, 5_000);
  watchdog.unref();
  try {
    await webServerHandle?.close();
    shardBridgeHandle?.close();
    await Promise.all(GuildPlayerManager.all().map((player) => player.stop()));
    await client.destroy();
  } catch (err) {
    logger.error({ err }, 'Error during shutdown cleanup');
  } finally {
    clearTimeout(watchdog);
    process.exit(exitCode);
  }
}
process.on('SIGINT', () => void shutdown('SIGINT'));
process.on('SIGTERM', () => void shutdown('SIGTERM'));

await client.login(env.discordToken);

// loadWebEnv returns null when WEB_DASHBOARD_ENABLED is off, so src/web (and its DB) is never imported.
const webEnv = loadWebEnv(env.clientId, resolveClientDir(path.dirname(fileURLToPath(import.meta.url))));
if (webEnv) {
  // ShardingManager sets process.env.SHARDS to this child's shard id;
  // undefined/'single' means a standalone process.
  const isShardChild = process.env.SHARDS !== undefined && process.env.SHARDS !== 'single';
  if (isShardChild) {
    // Under sharding the HTTP server lives in the manager (shard.ts); the child only exposes its players via the globalThis bridge.
    const { installShardBridge } = await import('./web/shardBridgeGlobal.js');
    shardBridgeHandle = installShardBridge(client);
  } else {
    const { LocalBridge } = await import('./web/bridge/local.js');
    const { startWebServer } = await import('./web/index.js');
    webServerHandle = startWebServer({ bridge: new LocalBridge(client), env: webEnv });
  }
}
