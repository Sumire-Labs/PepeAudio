import { Client, GatewayIntentBits, Options, Partials } from 'discord.js';
import { generateDependencyReport } from '@discordjs/voice';
import { env } from './config/env.js';
import { initFfmpeg } from './config/ffmpegResolver.js';
import { setFfmpegCapabilities } from './config/ffmpegState.js';
import { initHrirProfiles } from './config/hrirProfilesState.js';
import { initPanelManager } from './ui/panelManager.js';
import { registerReadyEvent } from './events/ready.js';
import { registerInteractionCreateEvent } from './events/interactionCreate.js';
import { registerVoiceStateUpdateEvent } from './events/voiceStateUpdate.js';
import { registerErrorEvents } from './events/errorEvents.js';
import { startMetricsReporter } from './util/metricsReporter.js';
import * as GuildPlayerManager from './player/GuildPlayerManager.js';
import { logger } from './logger.js';
import './data/db.js'; // ensures the SQLite schema migration runs before anything else touches it

const ffmpegCapabilities = initFfmpeg();
setFfmpegCapabilities(ffmpegCapabilities);
initPanelManager(ffmpegCapabilities);
initHrirProfiles(ffmpegCapabilities.path, env.hrirProfilesDirOverride);

// `@discordjs/opus` and `sodium-native` are optionalDependencies (see
// package.json) - if their native build fails to install, @discordjs/voice
// silently falls back to opusscript/libsodium-wrappers (pure JS), which is
// several times slower per stream. This is invisible without checking, so
// surface it explicitly rather than let it quietly degrade throughput at
// scale. Note: the report always prints each package's *name* whether found
// or not (`- name: <version|not found>`), so checking for the name alone
// would never actually detect a missing native module - the check has to
// look for the literal "not found" line for that specific package.
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
  // Never let message CONTENT trigger a mention notification. Track titles and
  // artist names come from external metadata (a YouTube video an attacker can
  // name "@everyone" or "<@&roleId>") and are echoed into PUBLIC messages — the
  // queued-added confirmation card and the now-playing panel. escapeMd (see
  // panelMarkdown.ts) only neutralizes markdown, not mention tokens, so without
  // this Discord's default (parse every mention in content) would let a crafted
  // title ping @everyone/a role/a user. `parse: []` still RENDERS mentions as
  // highlighted pills — including the panel's intentional "<@requester>" — but
  // suppresses the actual notification, which also stops the panel from
  // re-pinging the requester on every periodic refresh.
  allowedMentions: { parse: [] },
  makeCache: Options.cacheWithLimits({
    MessageManager: 0, // panelManager holds its own Message references directly from channel.send()'s return value - never re-fetched via this cache, so capping it to 0 has no effect on the panel.
    UserManager: 100,
    // GuildMemberManager deliberately left UNLIMITED (no entry here - the
    // conservative choice per instructions): voiceStateUpdate.ts's "is anyone
    // still in the VC" check reads channel.members, which is derived from
    // this exact cache. Verifying that a capped GuildMemberManager doesn't
    // silently break that alone-timeout detection requires a live Discord
    // connection with real members joining/leaving voice channels, which
    // isn't possible in this environment - see the final report's
    // human-verification checklist before enabling a limit here.
  }),
});

registerReadyEvent(client);
registerInteractionCreateEvent(client);
registerVoiceStateUpdateEvent(client);
startMetricsReporter(client);
// `shutdown` is a hoisted function declaration, so it's safe to reference here
// even though it's defined below; the callback only fires at runtime anyway.
registerErrorEvents(client, (reason, err) => {
  logger.error({ err, reason }, 'Fatal error - shutting down so the process can restart clean');
  void shutdown(reason, 1);
});

let shuttingDown = false;
async function shutdown(reason: string, exitCode = 0): Promise<void> {
  if (shuttingDown) return;
  shuttingDown = true;
  logger.info({ reason, exitCode }, 'Shutting down');
  // Cleanup must never wedge the exit: if stopping players / destroying the
  // client hangs (e.g. a stuck voice connection), force-exit after a grace
  // period so a supervised process still restarts.
  const watchdog = setTimeout(() => {
    logger.error('Graceful shutdown timed out - forcing exit');
    process.exit(exitCode === 0 ? 1 : exitCode);
  }, 5_000);
  watchdog.unref();
  try {
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
