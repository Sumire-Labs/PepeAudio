import { MessageFlags, type ChatInputCommandInteraction } from 'discord.js';
import type { GuildPlayer } from '../../player/GuildPlayer.js';
import type { QueueItem } from '../../player/QueueItem.js';
import { sendOrReplacePanel, type SendableChannel } from '../../ui/panelManager.js';
import { buildQueuedConfirmation } from '../../ui/panelBuilder.js';
import { logger } from '../../logger.js';

/**
 * Enqueues the resolved items onto the player and confirms the addition,
 * starting playback first if the player was idle. On the one recoverable
 * failure (queue at capacity) it edits the reply and returns; otherwise it
 * handles every remaining reply/confirmation itself.
 */
export async function enqueueAndConfirm(
  interaction: ChatInputCommandInteraction<'cached'>,
  player: GuildPlayer,
  items: QueueItem[],
): Promise<void> {
  const wasIdle = !player.currentTrack;
  const addedCount = player.enqueue(items);
  if (addedCount === 0) {
    await interaction.editReply({ content: 'キューが上限に達しているため追加できませんでした。` /skip` や `/stop` で整理してください。' });
    return;
  }

  // The deferred reply is only an ephemeral placeholder — every success path
  // below confirms via a public Components V2 card instead, so clear it now
  // rather than leaving an empty ephemeral message lingering.
  await interaction.deleteReply().catch(() => {});
  const channel = interaction.channel;
  const sendable: SendableChannel | null = channel && 'send' in channel ? channel : null;

  if (wasIdle) {
    await player.playNext();

    // Send/replace the live player panel. Playback already started, so a
    // panel-send hiccup shouldn't make the whole command look like it failed.
    if (sendable) {
      try {
        await sendOrReplacePanel(player, sendable);
      } catch (err) {
        logger.error({ err }, 'Failed to send/replace the player panel');
      }
    }
  }

  // Confirm the addition with a public Components V2 card — identical shape
  // whether this was the first track (shown just under the freshly-sent
  // panel) or an addition to an already-playing queue, so both cases look
  // the same. Sent publicly so everyone sees what got queued, not just the
  // requester.
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
  // The deferred reply was already deleted above, so on failure the
  // requester would otherwise see nothing at all despite the track(s)
  // genuinely having been enqueued - fall back to an ephemeral follow-up.
  if (!confirmationSent) {
    const firstTitle = queuedItems[0]?.title ?? '';
    const fallbackText =
      queuedItems.length === 1 ? `キューに追加しました: **${firstTitle}**` : `${queuedItems.length}曲をキューに追加しました。`;
    try {
      await interaction.followUp({ content: fallbackText, flags: MessageFlags.Ephemeral });
    } catch (err) {
      logger.error({ err }, 'Failed to send the fallback queue-added confirmation');
    }
  }
}
