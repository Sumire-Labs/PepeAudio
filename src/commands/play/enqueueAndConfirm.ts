import { MessageFlags, type RepliableInteraction } from 'discord.js';
import type { GuildPlayer } from '../../player/GuildPlayer.js';
import type { QueueItem } from '../../player/QueueItem.js';
import { sendOrReplacePanel, type SendableChannel } from '../../ui/panelManager.js';
import { buildQueuedConfirmation } from '../../ui/panelBuilder.js';
import { escapeMd } from '../../ui/panelMarkdown.js';
import { logger } from '../../logger.js';

/** Owns every reply/confirmation for the command — the caller must not reply after this. */
export async function enqueueAndConfirm(
  interaction: RepliableInteraction<'cached'>,
  player: GuildPlayer,
  items: QueueItem[],
): Promise<void> {
  const wasIdle = !player.currentTrack;
  const addedCount = player.enqueue(items);
  if (addedCount === 0) {
    await interaction.editReply({ content: 'キューが上限に達しているため追加できませんでした。` /skip` や `/stop` で整理してください。' });
    return;
  }

  // Success paths confirm via a public card, so drop the ephemeral placeholder.
  await interaction.deleteReply().catch(() => {});
  const channel = interaction.channel;
  const sendable: SendableChannel | null = channel && 'send' in channel ? channel : null;

  if (wasIdle) {
    await player.playNext();

    // Playback already started; a panel-send failure must not fail the command.
    if (sendable) {
      try {
        await sendOrReplacePanel(player, sendable);
      } catch (err) {
        logger.error({ err }, 'Failed to send/replace the player panel');
      }
    }
  }

  const queuedItems = addedCount < items.length ? items.slice(0, addedCount) : items;
  let confirmationSent = false;
  if (sendable) {
    try {
      await sendable.send({
        flags: [MessageFlags.IsComponentsV2, MessageFlags.SuppressNotifications],
        components: [buildQueuedConfirmation(queuedItems)],
      });
      confirmationSent = true;
    } catch (err) {
      logger.error({ err }, 'Failed to send the queue-added confirmation');
    }
  }
  // The deferred reply was deleted, so without this the requester sees nothing.
  if (!confirmationSent) {
    const firstTitle = queuedItems[0]?.title ?? '';
    const fallbackText =
      queuedItems.length === 1
        ? `キューに追加しました: **${escapeMd(firstTitle)}**`
        : `${queuedItems.length}曲をキューに追加しました。`;
    try {
      await interaction.followUp({ content: fallbackText, flags: MessageFlags.Ephemeral });
    } catch (err) {
      logger.error({ err }, 'Failed to send the fallback queue-added confirmation');
    }
  }
}
