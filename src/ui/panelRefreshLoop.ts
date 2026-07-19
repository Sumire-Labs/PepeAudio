import type { GuildPlayer } from '../player/GuildPlayer.js';
import * as GuildPlayerManager from '../player/GuildPlayerManager.js';
import { PANEL_PERIODIC_REFRESH_MS, panelRefreshIntervalMs } from '../player/constants.js';
import { lastEditAt, subscribedGuilds } from './panelState.js';
import { editPanel, scheduleCoalescedEdit } from './panelEdit.js';

export function ensureRefreshLoop(player: GuildPlayer): void {
  if (player.panelRefreshTimer) return;
  // Fixed tick; load throttling happens via the per-edit gap check below, not by re-arming the timer.
  player.panelRefreshTimer = setInterval(() => {
    if (player.destroyed) {
      stopRefreshLoop(player);
      return;
    }
    if (player.status !== 'playing') return;
    const activeCount = GuildPlayerManager.all().filter((p) => p.status === 'playing').length;
    const elapsedSinceLast = Date.now() - (lastEditAt.get(player.guildId) ?? 0);
    if (elapsedSinceLast < panelRefreshIntervalMs(activeCount)) return;
    void editPanel(player);
  }, PANEL_PERIODIC_REFRESH_MS);
}

export function stopRefreshLoop(player: GuildPlayer): void {
  if (player.panelRefreshTimer) {
    clearInterval(player.panelRefreshTimer);
    player.panelRefreshTimer = null;
  }
}

export function attachUpdateListener(player: GuildPlayer): void {
  if (subscribedGuilds.has(player.guildId)) return;
  subscribedGuilds.add(player.guildId);

  player.on('update', () => scheduleCoalescedEdit(player));
  player.once('destroyed', () => {
    subscribedGuilds.delete(player.guildId);
    stopRefreshLoop(player);
    // Don't touch panelMessages here: stop()'s final 'update' already routed through editPanel()'s idle branch, which deletes the panel and clears it.
  });
}
