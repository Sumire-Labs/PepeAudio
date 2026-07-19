import type { Guild } from 'discord.js';
import type { GuildPlayer } from '../../player/GuildPlayer.js';
import type { QueueItem } from '../../player/QueueItem.js';
import { getHrirProfiles } from '../../config/hrirProfilesState.js';
import { AURA_ENABLED } from '../../player/constants.js';
import type { GuildSnapshot, QueueItemDTO, ViewerCapabilities } from './types.js';

const HISTORY_DTO_LIMIT = 20;

export const DISPLAY_ONLY_VIEWER: ViewerCapabilities = {
  canControl: false,
  denyReason: null,
  inBotVoiceChannel: false,
};

/** Cache-only (no fetch). */
function resolveRequester(guild: Guild | undefined, userId: string): { name: string | null; avatarUrl: string | null } {
  const member = guild?.members.cache.get(userId);
  if (!member) return { name: null, avatarUrl: null };
  return { name: member.displayName, avatarUrl: member.user.displayAvatarURL({ size: 32 }) };
}

export function toQueueItemDTO(item: QueueItem, guild?: Guild): QueueItemDTO {
  const requester = resolveRequester(guild, item.requestedBy);
  return {
    id: item.id,
    title: item.title,
    artist: item.artist,
    durationMs: item.durationMs,
    thumbnailUrl: item.thumbnailUrl,
    sourceType: item.sourceType,
    sourceUrl: item.sourceUrl,
    requestedBy: item.requestedBy,
    requesterName: requester.name,
    requesterAvatarUrl: requester.avatarUrl,
  };
}

export function buildSnapshot(player: GuildPlayer, viewer: ViewerCapabilities, guild?: Guild): GuildSnapshot {
  const history = player.history;
  const trimmedHistory =
    history.length > HISTORY_DTO_LIMIT ? history.slice(history.length - HISTORY_DTO_LIMIT) : history;

  return {
    guildId: player.guildId,
    status: player.status,
    current: player.currentTrack ? toQueueItemDTO(player.currentTrack, guild) : null,
    elapsedMs: player.getElapsedMs(),
    queue: player.queue.map((item) => toQueueItemDTO(item, guild)),
    history: trimmedHistory.map((item) => toQueueItemDTO(item, guild)),
    loopMode: player.loopMode,
    shuffleEnabled: player.shuffleEnabled,
    autoplay: player.autoplay,
    volume: player.volume,
    hrirMode: player.hrirMode,
    aura360Mode: player.aura360Mode,
    hrirProfile: player.hrirProfile,
    auraPresets: getHrirProfiles().map((p) => ({ id: p.id, label: p.id.replace(/_/g, ' ') })),
    stay247: player.stay247,
    permissionMode: player.permissionMode,
    voiceChannelId: player.voiceChannelId,
    lastError: player.lastError,
    auraEnabled: AURA_ENABLED,
    viewer,
  };
}
