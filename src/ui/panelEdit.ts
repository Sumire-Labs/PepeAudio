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
 * Edits the panel message. Guarded so at most one message.edit() call is ever
 * in flight per guild at a time — two overlapping edits (e.g. the periodic
 * refresh tick landing while a button-triggered edit is still in flight)
 * could otherwise resolve out of order and leave a stale render as the last
 * word. If a render request comes in while one is in flight, exactly one
 * trailing edit fires afterward (picking up whatever the latest state is by
 * then) rather than either dropping it or stacking unbounded edits.
 */
export async function editPanel(player: GuildPlayer): Promise<void> {
  const message = panelMessages.get(player.guildId);
  if (!message || message.id !== player.panelMessageId) return; // stale/out-of-date guard

  if (editInFlight.has(player.guildId)) {
    editPendingAfterInFlight.add(player.guildId);
    return;
  }

  editInFlight.add(player.guildId);
  lastEditAt.set(player.guildId, Date.now());

  if (player.status === 'idle') {
    // Nothing playing/queued (natural queue exhaustion or /stop) - remove the
    // stale panel entirely rather than leaving a disabled "no track" message
    // cluttering the channel. The next /play or /now sends a fresh one.
    try {
      await message.delete();
    } catch (err) {
      const code = (err as { code?: number })?.code;
      if (code !== 10008) {
        logger.warn({ err, guildId: player.guildId }, 'Failed to delete idle panel message');
      }
    }
    // Re-check after the await: a concurrent sendOrReplacePanel() may have
    // already installed a BRAND NEW panel while the delete above was in
    // flight (e.g. a /play right as the queue emptied). If panelMessages no
    // longer points at the message we started with, this cleanup must not
    // clobber the new panel's bookkeeping.
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
  if (pendingEditTimers.has(guildId)) return; // an edit is already queued

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
