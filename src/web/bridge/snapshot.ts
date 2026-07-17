/**
 * Pure DTO builder: turns a live GuildPlayer into a JSON-serializable
 * GuildSnapshot. Runs only in the shard/single process (it touches GuildPlayer),
 * and is imported by both LocalBridge and the shard-side IPC push installer, so
 * both paths produce byte-identical snapshots.
 */
import type { GuildPlayer } from '../../player/GuildPlayer.js';
import type { QueueItem } from '../../player/QueueItem.js';
import { getHrirProfiles } from '../../config/hrirProfilesState.js';
import { AURA_ENABLED } from '../../player/constants.js';
import type { GuildSnapshot, QueueItemDTO, ViewerCapabilities } from './types.js';

/** How many of the most-recent history entries to include in a snapshot (keeps SSE frames small). */
const HISTORY_DTO_LIMIT = 20;

/** A placeholder for pushes that intentionally omit per-viewer capabilities (see BotBridge.subscribe). */
export const DISPLAY_ONLY_VIEWER: ViewerCapabilities = {
  canControl: false,
  denyReason: null,
  inBotVoiceChannel: false,
};

export function toQueueItemDTO(item: QueueItem): QueueItemDTO {
  return {
    id: item.id,
    title: item.title,
    artist: item.artist,
    durationMs: item.durationMs,
    thumbnailUrl: item.thumbnailUrl,
    sourceType: item.sourceType,
    sourceUrl: item.sourceUrl,
    requestedBy: item.requestedBy,
  };
}

/**
 * Builds the snapshot the browser renders. `viewer` is supplied by the caller
 * (LocalBridge computes real per-viewer caps; the shard IPC push uses
 * DISPLAY_ONLY_VIEWER). Everything read here is a plain getter on GuildPlayer —
 * no side effects, no playback mutation.
 */
export function buildSnapshot(player: GuildPlayer, viewer: ViewerCapabilities): GuildSnapshot {
  const history = player.history;
  const trimmedHistory =
    history.length > HISTORY_DTO_LIMIT ? history.slice(history.length - HISTORY_DTO_LIMIT) : history;

  return {
    guildId: player.guildId,
    status: player.status,
    current: player.currentTrack ? toQueueItemDTO(player.currentTrack) : null,
    elapsedMs: player.getElapsedMs(),
    queue: player.queue.map(toQueueItemDTO),
    history: trimmedHistory.map(toQueueItemDTO),
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
    serverTimeMs: Date.now(),
  };
}
