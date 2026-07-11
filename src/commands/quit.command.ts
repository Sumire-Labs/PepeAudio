import { MessageFlags, SlashCommandBuilder, type ChatInputCommandInteraction } from 'discord.js';
import * as GuildPlayerManager from '../player/GuildPlayerManager.js';
import { checkControlPermission } from '../ui/permissions.js';
import type { BotCommand } from './types.js';

export const quitCommand: BotCommand = {
  data: new SlashCommandBuilder()
    .setName('quit')
    .setDescription('ボイスチャンネルから退出します（24/7モード中でも強制的に退出します）'),

  async execute(interaction: ChatInputCommandInteraction): Promise<void> {
    if (!interaction.inCachedGuild()) {
      await interaction.reply({ content: 'サーバー内でのみ使用できます。', flags: MessageFlags.Ephemeral });
      return;
    }

    const player = GuildPlayerManager.get(interaction.guildId);
    if (!player || player.destroyed) {
      await interaction.reply({ content: 'Botはボイスチャンネルに参加していません。', flags: MessageFlags.Ephemeral });
      return;
    }

    const perm = checkControlPermission(interaction, player);
    if (!perm.ok) {
      await interaction.reply({ content: perm.reason ?? '権限がありません。', flags: MessageFlags.Ephemeral });
      return;
    }

    await interaction.deferReply({ flags: MessageFlags.Ephemeral });
    await player.stop();
    await interaction.editReply({ content: '退出しました。' });
  },
};
