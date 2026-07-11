import type { Message, MessageCreateOptions } from 'discord.js';
import type { GuildPlayer } from '../player/GuildPlayer.js';
import { logger } from '../logger.js';
import { renderSendOptions } from './panelRender.js';
import { panelMessages, lastEditAt } from './panelState.js';
import { attachUpdateListener, ensureRefreshLoop } from './panelRefreshLoop.js';

export { initPanelManager } from './panelState.js';
export { editPanel } from './panelEdit.js';

export interface SendableChannel {
  id: string;
  send(options: MessageCreateOptions): Promise<Message>;
}

/**
 * Sends a brand-new panel message, then deletes whatever panel preceded it
 * for this guild (new message first, delete second — closes the race where a
 * button press on the old message lands while the delete call is in flight,
 * since interactionCreate compares against player.panelMessageId which is
 * reassigned below before the delete happens).
 */
export async function sendOrReplacePanel(player: GuildPlayer, channel: SendableChannel): Promise<void> {
  const previousMessage = panelMessages.get(player.guildId);

  const newMessage = await channel.send(renderSendOptions(player));

  player.panelMessageId = newMessage.id;
  player.panelChannelId = newMessage.channelId;
  player.textChannelId = newMessage.channelId;
  panelMessages.set(player.guildId, newMessage);
  lastEditAt.set(player.guildId, Date.now());

  if (previousMessage && previousMessage.id !== newMessage.id) {
    previousMessage.delete().catch((err: unknown) => {
      logger.warn({ err, guildId: player.guildId }, 'Failed to delete previous panel message (non-fatal)');
    });
  }

  attachUpdateListener(player);
  ensureRefreshLoop(player);
}
