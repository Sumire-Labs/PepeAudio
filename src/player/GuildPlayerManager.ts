import { EventEmitter } from 'node:events';
import type { DiscordGatewayAdapterCreator } from '@discordjs/voice';
import { GuildPlayer } from './GuildPlayer.js';
import type { FfmpegCapabilities } from '../config/ffmpegResolver.js';

const players = new Map<string, GuildPlayer>();

/**
 * Fires `'created'` with the freshly-constructed GuildPlayer whenever
 * getOrCreate() builds a new one. The only consumer is the web dashboard's
 * LocalBridge (src/web/bridge/local.ts): a browser can open a guild's panel
 * before any `/play` has created that guild's player, so the bridge listens
 * here to attach its realtime `'update'` listener the moment the player comes
 * into existence. Nothing in the core bot depends on this — it's a no-op when
 * the dashboard is disabled (no listeners attached).
 */
export const events = new EventEmitter();

export interface GetOrCreateParams {
  guildId: string;
  textChannelId: string;
  voiceChannelId: string;
  adapterCreator: DiscordGatewayAdapterCreator;
  ffmpeg: FfmpegCapabilities;
}

/** Every command/button/select handler looks the player up here — never holds its own reference. */
export function get(guildId: string): GuildPlayer | undefined {
  return players.get(guildId);
}

export function has(guildId: string): boolean {
  const player = players.get(guildId);
  return Boolean(player) && !player!.destroyed;
}

export function getOrCreate(params: GetOrCreateParams): GuildPlayer {
  const existing = players.get(params.guildId);
  if (existing && !existing.destroyed) return existing;

  const player = new GuildPlayer(params);
  player.once('destroyed', () => {
    if (players.get(params.guildId) === player) {
      players.delete(params.guildId);
    }
  });
  players.set(params.guildId, player);
  events.emit('created', player);
  return player;
}

export async function destroy(guildId: string): Promise<void> {
  const player = players.get(guildId);
  if (player) await player.stop();
}

export function all(): GuildPlayer[] {
  return [...players.values()];
}
