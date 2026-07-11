import { MessageFlags, type ModalSubmitInteraction } from 'discord.js';
import * as GuildPlayerManager from '../player/GuildPlayerManager.js';
import { checkControlPermission } from '../ui/permissions.js';
import { parseAddQueueModalId } from '../ui/customIds.js';
import { ADD_QUEUE_QUERY_FIELD_ID } from '../ui/addQueueModal.js';
import { resolvePlayQuery } from '../commands/play/resolveQuery.js';
import { enqueueAndConfirm } from '../commands/play/enqueueAndConfirm.js';

/**
 * Handles the modal shown by the panel's "➕ 曲を追加" button (see
 * ui/addQueueModal.ts). Reuses /play's own query-resolution/enqueue logic
 * (resolvePlayQuery/enqueueAndConfirm) rather than duplicating it.
 */
export async function handleAddQueueModalSubmit(interaction: ModalSubmitInteraction): Promise<void> {
  const parsed = parseAddQueueModalId(interaction.customId);
  if (!parsed) return; // not ours

  if (!interaction.inCachedGuild() || interaction.guildId !== parsed.guildId) {
    await interaction.reply({ content: '不正な操作です。', flags: MessageFlags.Ephemeral });
    return;
  }

  const player = GuildPlayerManager.get(parsed.guildId);
  if (!player || player.destroyed) {
    await interaction.reply({ content: 'このパネルは無効です。`/now` で再表示してください。', flags: MessageFlags.Ephemeral });
    return;
  }

  // Re-check staleness/permission at submit time too - filling out a modal can
  // take a while, during which the panel could have been replaced or the user
  // could have left the voice channel. ModalSubmitInteraction.message is the
  // origin message the modal was launched from (present here since it's always
  // launched from a button on the panel message).
  if (interaction.message && interaction.message.id !== player.panelMessageId) {
    await interaction.reply({ content: 'このパネルは古くなっています。最新のパネルをご利用ください。', flags: MessageFlags.Ephemeral });
    return;
  }

  const perm = checkControlPermission(interaction, player);
  if (!perm.ok) {
    await interaction.reply({ content: perm.reason ?? '権限がありません。', flags: MessageFlags.Ephemeral });
    return;
  }

  const query = interaction.fields.getTextInputValue(ADD_QUEUE_QUERY_FIELD_ID);

  await interaction.deferReply({ flags: MessageFlags.Ephemeral });

  const items = await resolvePlayQuery(query, interaction.user.id, interaction);
  if (!items) return;

  await enqueueAndConfirm(interaction, player, items);
}
