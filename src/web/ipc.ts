// The bridge is stashed on globalThis because broadcastEval runs as a stringified
// eval that can't close over manager-side imports.
import type { CommandResult, GuildSnapshot, GuildSummary, ResolveResult, SearchCandidate, WebCommand } from './bridge/types.js';

export interface PepeShardBridge {
  getSnapshot(guildId: string, userId: string): Promise<GuildSnapshot | null>;
  runCommand(guildId: string, userId: string, command: WebCommand): Promise<CommandResult>;
  search(query: string): Promise<SearchCandidate[]>;
  resolveTracks(query: string): Promise<ResolveResult>;
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
