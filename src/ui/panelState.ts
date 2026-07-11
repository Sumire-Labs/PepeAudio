import type { Message } from 'discord.js';
import type { FfmpegCapabilities } from '../config/ffmpegResolver.js';

let ffmpegCaps: FfmpegCapabilities | null = null;
export function initPanelManager(caps: FfmpegCapabilities): void {
  ffmpegCaps = caps;
}
export function getFfmpegCaps(): FfmpegCapabilities {
  if (!ffmpegCaps) throw new Error('panelManager not initialized — call initPanelManager() at startup');
  return ffmpegCaps;
}

/** Live Message references, keyed by guildId — used for both editing and "delete the old one" replacement. */
export const panelMessages = new Map<string, Message>();
export const lastEditAt = new Map<string, number>();
export const pendingEditTimers = new Map<string, NodeJS.Timeout>();
export const subscribedGuilds = new Set<string>();
/** Guards against two overlapping message.edit() calls for the same panel (they could resolve out of order). */
export const editInFlight = new Set<string>();
export const editPendingAfterInFlight = new Set<string>();

export const MIN_EDIT_GAP_MS = 750;
