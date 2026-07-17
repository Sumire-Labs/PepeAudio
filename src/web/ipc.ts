/**
 * The IPC contract between shard children and the ShardingManager for the web
 * dashboard, plus the `globalThis.__pepeBridge` type shared by both sides.
 *
 * - The shard side (shardBridgeGlobal.ts) stashes a PepeShardBridge on globalThis
 *   so the manager's broadcastEval'd functions can reach it (a stringified eval
 *   can't close over manager-side imports).
 * - The shard side pushes ShardToManagerMessage over client.shard.send(); the
 *   manager (ShardedBridge) receives them via shard.on('message').
 */
import type { CommandResult, GuildSnapshot, GuildSummary, WebCommand } from './bridge/types.js';

/** The surface the manager invokes on each shard via broadcastEval. */
export interface PepeShardBridge {
  getSnapshot(guildId: string, userId: string): Promise<GuildSnapshot | null>;
  runCommand(guildId: string, userId: string, command: WebCommand): Promise<CommandResult>;
  listActive(userGuildIds: string[]): GuildSummary[];
  setWebSubscribed(guildId: string, on: boolean): void;
}

declare global {
  var __pepeBridge: PepeShardBridge | undefined;
}

export const PEPE_UPDATE = 'pepe:update';
export const PEPE_DESTROYED = 'pepe:destroyed';

export type ShardToManagerMessage =
  | { type: typeof PEPE_UPDATE; guildId: string; snapshot: GuildSnapshot }
  | { type: typeof PEPE_DESTROYED; guildId: string };

export function isShardToManagerMessage(msg: unknown): msg is ShardToManagerMessage {
  if (!msg || typeof msg !== 'object') return false;
  const type = (msg as { type?: unknown }).type;
  return (type === PEPE_UPDATE || type === PEPE_DESTROYED) && typeof (msg as { guildId?: unknown }).guildId === 'string';
}
