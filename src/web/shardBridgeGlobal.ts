/**
 * Runs inside each shard child (an index.ts process under the ShardingManager).
 * Stashes a PepeShardBridge on globalThis so the manager's broadcastEval'd
 * functions can reach this shard's GuildPlayerManager, and pushes throttled
 * player-state updates back to the manager over the shard IPC channel — but only
 * for guilds a browser is actually watching (tracked in `webSubscribed`), so
 * idle guilds generate zero IPC traffic.
 */
import type { Client } from 'discord.js';
import * as GuildPlayerManager from '../player/GuildPlayerManager.js';
import type { GuildPlayer } from '../player/GuildPlayer.js';
import { logger } from '../logger.js';
import { throttle } from './util/throttle.js';
import { resolveViewerCapabilities } from './bridge/permission.js';
import { buildSnapshot, DISPLAY_ONLY_VIEWER } from './bridge/snapshot.js';
import { runWebCommand, runWebResolve, runWebSearch } from './bridge/command.js';
import { PEPE_DESTROYED, PEPE_UPDATE, type PepeShardBridge, type ShardToManagerMessage } from './ipc.js';
import type { GuildSummary } from './bridge/types.js';

const PUSH_THROTTLE_MS = 750;

export interface ShardBridgeHandle {
  close(): void;
}

export function installShardBridge(client: Client): ShardBridgeHandle {
  const webSubscribed = new Set<string>();
  const attached = new WeakSet<GuildPlayer>();

  const send = (msg: ShardToManagerMessage): void => {
    const pr = client.shard?.send(msg);
    if (pr) void pr.catch((err) => logger.debug({ err }, 'shard.send to manager failed'));
  };

  const pushSnapshot = (guildId: string): void => {
    const player = GuildPlayerManager.get(guildId);
    if (!player || player.destroyed) return;
    const guild = client.guilds.cache.get(guildId);
    send({ type: PEPE_UPDATE, guildId, snapshot: buildSnapshot(player, DISPLAY_ONLY_VIEWER, guild) });
  };

  const attachPush = (player: GuildPlayer): void => {
    if (attached.has(player)) return;
    attached.add(player);
    const onUpdate = throttle(() => {
      if (webSubscribed.has(player.guildId)) pushSnapshot(player.guildId);
    }, PUSH_THROTTLE_MS);
    player.on('update', onUpdate);
    player.once('destroyed', () => {
      if (webSubscribed.has(player.guildId)) send({ type: PEPE_DESTROYED, guildId: player.guildId });
    });
  };

  // Attach to already-live players and any created later.
  for (const player of GuildPlayerManager.all()) attachPush(player);
  const onCreated = (player: GuildPlayer): void => attachPush(player);
  GuildPlayerManager.events.on('created', onCreated);

  const bridge: PepeShardBridge = {
    async getSnapshot(guildId, userId) {
      const player = GuildPlayerManager.get(guildId);
      if (!player || player.destroyed) return null;
      const guild = client.guilds.cache.get(guildId);
      const caps = await resolveViewerCapabilities(guild, userId, player);
      return buildSnapshot(player, caps, guild);
    },
    async runCommand(guildId, userId, command) {
      return runWebCommand(guildId, userId, command, client);
    },
    async search(query) {
      return runWebSearch(query);
    },
    async resolveTracks(query) {
      return runWebResolve(query);
    },
    listActive(userGuildIds) {
      const memberOf = new Set(userGuildIds);
      const result: GuildSummary[] = [];
      for (const guild of client.guilds.cache.values()) {
        if (!memberOf.has(guild.id)) continue;
        const player = GuildPlayerManager.get(guild.id);
        const active = Boolean(player && !player.destroyed);
        result.push({
          guildId: guild.id,
          name: guild.name,
          iconUrl: guild.iconURL({ size: 128 }) ?? null,
          hasActiveSession: active,
          status: active ? player!.status : 'idle',
          currentTitle: active ? (player!.currentTrack?.title ?? null) : null,
        });
      }
      return result;
    },
    setWebSubscribed(guildId, on) {
      if (on) {
        webSubscribed.add(guildId);
        // Push current state immediately so the browser sees it without waiting.
        pushSnapshot(guildId);
      } else {
        webSubscribed.delete(guildId);
      }
    },
  };

  globalThis.__pepeBridge = bridge;
  logger.info('Shard web bridge installed');

  return {
    close(): void {
      GuildPlayerManager.events.off('created', onCreated);
      if (globalThis.__pepeBridge === bridge) globalThis.__pepeBridge = undefined;
      webSubscribed.clear();
    },
  };
}
