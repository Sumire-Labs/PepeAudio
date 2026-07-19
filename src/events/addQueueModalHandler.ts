import { MessageFlags, type ModalSubmitInteraction } from 'discord.js';
import * as GuildPlayerManager from '../player/GuildPlayerManager.js';
import { checkControlPermission } from '../ui/permissions.js';
import { parseAddQueueModalId } from '../ui/customIds.js';
import { ADD_QUEUE_QUERY_FIELD_ID } from '../ui/addQueueModal.js';
import { resolvePlayQuery } from '../commands/play/resolveQuery.js';
import { enqueueAndConfirm } from '../commands/play/enqueueAndConfirm.js';
import { checkCooldown } from '../util/rateLimiter.js';
import { PLAY_COOLDOWN_MS } from '../player/constants.js';

export async function handleAddQueueModalSubmit(interaction: ModalSubmitInteraction): Promise<void> {
  const parsed = parseAddQueueModalId(interaction.customId);
  if (!parsed) return;

  if (!interaction.inCachedGuild() || interaction.guildId !== parsed.guildId) {
    await interaction.reply({ content: '不正な操作です。', flags: MessageFlags.Ephemeral });
    return;
  }

  const player = GuildPlayerManager.get(parsed.guildId);
  if (!player || player.destroyed) {
    await interaction.reply({ content: 'このパネルは無効です。`/now` で再表示してください。', flags: MessageFlags.Ephemeral });
    return;
  }

  // Re-check staleness at submit: filling the modal takes time, so the panel may
  // have been replaced meanwhile. interaction.message is the panel message the
  // modal launched from.
  if (interaction.message && interaction.message.id !== player.panelMessageId) {
    await interaction.reply({ content: 'このパネルは古くなっています。最新のパネルをご利用ください。', flags: MessageFlags.Ephemeral });
    return;
  }

  const perm = checkControlPermission(interaction, player);
  if (!perm.ok) {
    await interaction.reply({ content: perm.reason ?? '権限がありません。', flags: MessageFlags.Ephemeral });
    return;
  }

  // Share /play's cooldown bucket: this modal hits the same expensive resolve
  // path, so a separate limit would let users alternate the two to defeat both.
  if (!checkCooldown('play', interaction.user.id, PLAY_COOLDOWN_MS)) {
    await interaction.reply({ content: '少し間隔を空けてから再度お試しください。', flags: MessageFlags.Ephemeral });
    return;
  }

  const query = interaction.fields.getTextInputValue(ADD_QUEUE_QUERY_FIELD_ID);

  await interaction.deferReply({ flags: MessageFlags.Ephemeral });

  const items = await resolvePlayQuery(query, interaction.user.id, interaction);
  if (!items) return;

  await enqueueAndConfirm(interaction, player, items);
}
