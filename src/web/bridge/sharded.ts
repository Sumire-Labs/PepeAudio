/**
 * BotBridge for sharded production (the ShardingManager process, which has NO
 * discord.js Client). Reaches the shard that owns a guild via broadcastEval, and
 * receives realtime pushes via shard IPC messages. Every eval'd function is a
 * fixed, server-authored closure over nothing but (client, context) + globalThis
 * — the only dynamic data crossing the boundary is the JSON context.
 */
import type { Shard, ShardingManager } from 'discord.js';
import { logger } from '../../logger.js';
import { isShardToManagerMessage, PEPE_UPDATE } from '../ipc.js';
import type { BotBridge, CommandResult, GuildSnapshot, GuildSummary, WebCommand } from './types.js';

type Subscriber = (snapshot: GuildSnapshot | null) => void;

export class ShardedBridge implements BotBridge {
  private readonly subs = new Map<string, Set<Subscriber>>();
  private readonly onShardCreate: (shard: Shard) => void;

  constructor(private readonly manager: ShardingManager) {
    for (const shard of manager.shards.values()) this.listenToShard(shard);
    // Shards respawned later must also be wired for IPC pushes.
    this.onShardCreate = (shard: Shard) => this.listenToShard(shard);
    manager.on('shardCreate', this.onShardCreate);
  }

  private listenToShard(shard: Shard): void {
    shard.on('message', (msg: unknown) => this.onShardMessage(msg));
  }

  private onShardMessage(msg: unknown): void {
    if (!isShardToManagerMessage(msg)) return;
    const set = this.subs.get(msg.guildId);
    if (!set || set.size === 0) return;
    const payload = msg.type === PEPE_UPDATE ? msg.snapshot : null;
    for (const cb of set) cb(payload);
  }

  private shardIdFor(guildId: string): number {
    const total = typeof this.manager.totalShards === 'number' ? this.manager.totalShards : 1;
    return Number((BigInt(guildId) >> 22n) % BigInt(total));
  }

  async listControllableGuilds(userGuildIds: string[], _userId: string): Promise<GuildSummary[]> {
    // Fan out to ALL shards — a guild can be on any — then flatten. Each guild is
    // hosted by exactly one shard, so there are no duplicates.
    const raw = (await this.manager
      .broadcastEval(
        (_client, ctx: { userGuildIds: string[] }) =>
          globalThis.__pepeBridge ? globalThis.__pepeBridge.listActive(ctx.userGuildIds) : [],
        { context: { userGuildIds } },
      )
      .catch((err) => {
        logger.debug({ err }, 'ShardedBridge.listControllableGuilds broadcastEval failed');
        return [];
      })) as unknown;
    if (!Array.isArray(raw)) return [];
    return (raw as GuildSummary[][]).flat();
  }

  async getSnapshot(guildId: string, userId: string): Promise<GuildSnapshot | null> {
    const raw = (await this.manager
      .broadcastEval(
        (_client, ctx: { guildId: string; userId: string }) =>
          globalThis.__pepeBridge ? globalThis.__pepeBridge.getSnapshot(ctx.guildId, ctx.userId) : null,
        { shard: this.shardIdFor(guildId), context: { guildId, userId } },
      )
      .catch(() => null)) as unknown;
    const value = Array.isArray(raw) ? raw[0] : raw;
    return (value ?? null) as GuildSnapshot | null;
  }

  async runCommand(guildId: string, userId: string, command: WebCommand): Promise<CommandResult> {
    const raw = (await this.manager
      .broadcastEval(
        (_client, ctx: { guildId: string; userId: string; command: WebCommand }) =>
          globalThis.__pepeBridge
            ? globalThis.__pepeBridge.runCommand(ctx.guildId, ctx.userId, ctx.command)
            : { ok: false, error: 'まだ起動中です。少し待ってから再度お試しください。' },
        { shard: this.shardIdFor(guildId), context: { guildId, userId, command } },
      )
      .catch((err) => {
        logger.error({ err, guildId, command: command.type }, 'ShardedBridge.runCommand broadcastEval failed');
        return { ok: false, error: '操作に失敗しました。もう一度お試しください。' } satisfies CommandResult;
      })) as unknown;
    const value = Array.isArray(raw) ? raw[0] : raw;
    return (value ?? { ok: false, error: '応答がありませんでした。' }) as CommandResult;
  }

  subscribe(guildId: string, _userId: string, cb: Subscriber): () => void {
    let set = this.subs.get(guildId);
    if (!set) {
      set = new Set();
      this.subs.set(guildId, set);
      this.setShardSubscribed(guildId, true);
    }
    set.add(cb);

    return () => {
      const current = this.subs.get(guildId);
      if (!current) return;
      current.delete(cb);
      if (current.size === 0) {
        this.subs.delete(guildId);
        this.setShardSubscribed(guildId, false);
      }
    };
  }

  private setShardSubscribed(guildId: string, on: boolean): void {
    void this.manager
      .broadcastEval(
        (_client, ctx: { guildId: string; on: boolean }) => {
          globalThis.__pepeBridge?.setWebSubscribed(ctx.guildId, ctx.on);
        },
        { shard: this.shardIdFor(guildId), context: { guildId, on } },
      )
      .catch(() => {
        // A shard mid-respawn may reject; the next subscribe re-issues this.
      });
  }

  close(): void {
    this.manager.off('shardCreate', this.onShardCreate);
    this.subs.clear();
  }
}
