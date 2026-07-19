import type { GuildPlayer } from '../player/GuildPlayer.js';
import { logger } from '../logger.js';
import { renderEditOptions } from './panelRender.js';
import {
  panelMessages,
  lastEditAt,
  pendingEditTimers,
  editInFlight,
  editPendingAfterInFlight,
  MIN_EDIT_GAP_MS,
} from './panelState.js';
import { stopRefreshLoop } from './panelRefreshLoop.js';

/**
 * At most one message.edit() in flight per guild — overlapping edits can resolve
 * out of order and leave a stale render as the last word. A mid-flight request
 * coalesces into one trailing edit.
 */
export async function editPanel(player: GuildPlayer): Promise<void> {
  const message = panelMessages.get(player.guildId);
  if (!message || message.id !== player.panelMessageId) return;

  if (editInFlight.has(player.guildId)) {
    editPendingAfterInFlight.add(player.guildId);
    return;
  }

  editInFlight.add(player.guildId);
  lastEditAt.set(player.guildId, Date.now());

  if (player.status === 'idle') {
    // Idle: delete the panel rather than leave a stale "no track" message; next /play or /now sends a fresh one.
    try {
      await message.delete();
    } catch (err) {
      const code = (err as { code?: number })?.code;
      if (code !== 10008) {
        logger.warn({ err, guildId: player.guildId }, 'Failed to delete idle panel message');
      }
    }
    // Re-check after the await: a concurrent sendOrReplacePanel() may have installed
    // a new panel while the delete was in flight; don't clobber its bookkeeping.
    if (panelMessages.get(player.guildId) === message) {
      stopRefreshLoop(player);
      panelMessages.delete(player.guildId);
      player.panelMessageId = null;
      player.panelChannelId = null;
    }
    editInFlight.delete(player.guildId);
    if (editPendingAfterInFlight.delete(player.guildId)) {
      void editPanel(player);
    }
    return;
  }

  try {
    await message.edit(renderEditOptions(player));
  } catch (err) {
    const code = (err as { code?: number })?.code;
    if (code === 10008) {
      // Unknown Message — someone deleted the panel out-of-band. Stop chasing it.
      logger.info({ guildId: player.guildId }, 'Panel message no longer exists - stopping refresh loop');
      // Same re-check as the idle branch above — don't clobber a panel a
      // concurrent sendOrReplacePanel() already installed while this edit was in flight.
      if (panelMessages.get(player.guildId) === message) {
        stopRefreshLoop(player);
        panelMessages.delete(player.guildId);
        player.panelMessageId = null;
        player.panelChannelId = null;
      }
      editInFlight.delete(player.guildId);
      if (editPendingAfterInFlight.delete(player.guildId)) {
        void editPanel(player);
      }
      return;
    }
    logger.warn({ err, guildId: player.guildId }, 'Failed to edit panel message');
  }
  editInFlight.delete(player.guildId);

  if (editPendingAfterInFlight.delete(player.guildId)) {
    void editPanel(player);
  }
}

export function scheduleCoalescedEdit(player: GuildPlayer): void {
  const guildId = player.guildId;
  if (pendingEditTimers.has(guildId)) return;

  const elapsedSinceLast = Date.now() - (lastEditAt.get(guildId) ?? 0);
  if (elapsedSinceLast >= MIN_EDIT_GAP_MS) {
    void editPanel(player);
    return;
  }
  const timer = setTimeout(() => {
    pendingEditTimers.delete(guildId);
    void editPanel(player);
  }, MIN_EDIT_GAP_MS - elapsedSinceLast);
  pendingEditTimers.set(guildId, timer);
}
