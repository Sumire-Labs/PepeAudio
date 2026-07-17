/**
 * BotBridge for single-process mode (npm run dev / start:single): the web server
 * shares the process with the Client and GuildPlayerManager, so every call is a
 * direct in-process method call. Realtime is wired via GuildPlayerManager's
 * 'created' emitter plus each player's own 'update'/'destroyed' events.
 */
import type { Client } from 'discord.js';
import * as GuildPlayerManager from '../../player/GuildPlayerManager.js';
import type { GuildPlayer } from '../../player/GuildPlayer.js';
import { logger } from '../../logger.js';
import { throttle } from '../util/throttle.js';
import { resolveViewerCapabilities } from './permission.js';
import { buildSnapshot } from './snapshot.js';
import { runWebCommand, runWebSearch } from './command.js';
import type { BotBridge, CommandResult, GuildSnapshot, GuildSummary, SearchCandidate, WebCommand } from './types.js';

const PUSH_THROTTLE_MS = 500;

interface Subscriber {
  userId: string;
  cb: (snapshot: GuildSnapshot | null) => void;
}

interface PlayerHooks {
  player: GuildPlayer;
  onUpdate: () => void;
  onDestroyed: () => void;
}

export class LocalBridge implements BotBridge {
  private readonly subs = new Map<string, Set<Subscriber>>();
  private readonly hooks = new Map<string, PlayerHooks>();
  private readonly onPlayerCreated: (player: GuildPlayer) => void;

  constructor(private readonly client: Client) {
    this.onPlayerCreated = (player: GuildPlayer) => {
      if (this.subs.has(player.guildId)) this.attach(player);
    };
    GuildPlayerManager.events.on('created', this.onPlayerCreated);
  }

  async listControllableGuilds(userGuildIds: string[], _userId: string): Promise<GuildSummary[]> {
    const result: GuildSummary[] = [];
    for (const guildId of userGuildIds) {
      const guild = this.client.guilds.cache.get(guildId);
      if (!guild) continue; // the bot isn't a member of this guild — can't control it
      const player = GuildPlayerManager.get(guildId);
      const active = Boolean(player && !player.destroyed);
      result.push({
        guildId,
        name: guild.name,
        iconUrl: guild.iconURL({ size: 128 }) ?? null,
        hasActiveSession: active,
        status: active ? player!.status : 'idle',
        currentTitle: active ? (player!.currentTrack?.title ?? null) : null,
      });
    }
    return result;
  }

  async getSnapshot(guildId: string, userId: string): Promise<GuildSnapshot | null> {
    const player = GuildPlayerManager.get(guildId);
    if (!player || player.destroyed) return null;
    const guild = this.client.guilds.cache.get(guildId);
    const caps = await resolveViewerCapabilities(guild, userId, player);
    return buildSnapshot(player, caps, guild);
  }

  async runCommand(guildId: string, userId: string, command: WebCommand): Promise<CommandResult> {
    return runWebCommand(guildId, userId, command, this.client);
  }

  async search(query: string): Promise<SearchCandidate[]> {
    return runWebSearch(query);
  }

  subscribe(guildId: string, userId: string, cb: (snapshot: GuildSnapshot | null) => void): () => void {
    const subscriber: Subscriber = { userId, cb };
    let set = this.subs.get(guildId);
    if (!set) {
      set = new Set();
      this.subs.set(guildId, set);
    }
    set.add(subscriber);

    // Attach to an already-live player (a later-created one is caught by onPlayerCreated).
    const player = GuildPlayerManager.get(guildId);
    if (player && !player.destroyed) {
      this.attach(player);
      void this.pushOne(player, subscriber);
    }

    return () => {
      const current = this.subs.get(guildId);
      if (!current) return;
      current.delete(subscriber);
      if (current.size === 0) {
        this.subs.delete(guildId);
        this.detach(guildId);
      }
    };
  }

  private attach(player: GuildPlayer): void {
    const existing = this.hooks.get(player.guildId);
    if (existing && existing.player === player) return;
    if (existing) this.removeHooks(existing);

    const onUpdate = throttle(() => this.pushAll(player.guildId), PUSH_THROTTLE_MS);
    const onDestroyed = (): void => {
      const set = this.subs.get(player.guildId);
      if (set) for (const sub of set) sub.cb(null);
      this.detach(player.guildId);
    };
    player.on('update', onUpdate);
    player.once('destroyed', onDestroyed);
    this.hooks.set(player.guildId, { player, onUpdate, onDestroyed });

    // Push an immediate snapshot so a just-created (or newly-subscribed) player's
    // current state reaches the browser without waiting for the next update.
    this.pushAll(player.guildId);
  }

  private detach(guildId: string): void {
    const hook = this.hooks.get(guildId);
    if (hook) {
      this.removeHooks(hook);
      this.hooks.delete(guildId);
    }
  }

  private removeHooks(hook: PlayerHooks): void {
    hook.player.off('update', hook.onUpdate);
    hook.player.off('destroyed', hook.onDestroyed);
  }

  private pushAll(guildId: string): void {
    const set = this.subs.get(guildId);
    const player = GuildPlayerManager.get(guildId);
    if (!set || !player || player.destroyed) return;
    for (const sub of set) void this.pushOne(player, sub);
  }

  private async pushOne(player: GuildPlayer, sub: Subscriber): Promise<void> {
    try {
      const guild = this.client.guilds.cache.get(player.guildId);
      const caps = await resolveViewerCapabilities(guild, sub.userId, player);
      sub.cb(buildSnapshot(player, caps, guild));
    } catch (err) {
      logger.debug({ err, guildId: player.guildId }, 'LocalBridge: failed to push a snapshot');
    }
  }

  close(): void {
    GuildPlayerManager.events.off('created', this.onPlayerCreated);
    for (const hook of this.hooks.values()) this.removeHooks(hook);
    this.hooks.clear();
    this.subs.clear();
  }
}
