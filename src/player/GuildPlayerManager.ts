import type { DiscordGatewayAdapterCreator } from '@discordjs/voice';
import { GuildPlayer } from './GuildPlayer.js';
import type { FfmpegCapabilities } from '../config/ffmpegResolver.js';

const players = new Map<string, GuildPlayer>();

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
  return player;
}

export async function destroy(guildId: string): Promise<void> {
  const player = players.get(guildId);
  if (player) await player.stop();
}

export function all(): GuildPlayer[] {
  return [...players.values()];
}
