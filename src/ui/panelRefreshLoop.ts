import type { GuildPlayer } from '../player/GuildPlayer.js';
import * as GuildPlayerManager from '../player/GuildPlayerManager.js';
import { PANEL_PERIODIC_REFRESH_MS, panelRefreshIntervalMs } from '../player/constants.js';
import { lastEditAt, subscribedGuilds } from './panelState.js';
import { editPanel, scheduleCoalescedEdit } from './panelEdit.js';

export function ensureRefreshLoop(player: GuildPlayer): void {
  if (player.panelRefreshTimer) return;
  // The setInterval tick itself stays fixed at PANEL_PERIODIC_REFRESH_MS
  // (10s) - no timer rescheduling needed. What varies with load is how many
  // ticks actually result in an edit: panelRefreshIntervalMs() scales the
  // minimum gap up as more guilds are simultaneously playing, so this reacts
  // to load changes immediately rather than needing to re-arm a timer.
  player.panelRefreshTimer = setInterval(() => {
    if (player.destroyed) {
      stopRefreshLoop(player);
      return;
    }
    if (player.status !== 'playing') return; // only tick while actually playing
    const activeCount = GuildPlayerManager.all().filter((p) => p.status === 'playing').length;
    const elapsedSinceLast = Date.now() - (lastEditAt.get(player.guildId) ?? 0);
    if (elapsedSinceLast < panelRefreshIntervalMs(activeCount)) return; // a discrete-event edit just happened, or load-throttled
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
    // Not touching panelMessages here: stop() already emitted a final 'update'
    // just before this, which editPanel() (idle branch) handles by deleting
    // the panel itself and clearing panelMessages/panelMessageId.
  });
}
