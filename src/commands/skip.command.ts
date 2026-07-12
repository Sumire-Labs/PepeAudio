import { MessageFlags, SlashCommandBuilder, type ChatInputCommandInteraction } from 'discord.js';
import * as GuildPlayerManager from '../player/GuildPlayerManager.js';
import { checkControlPermission } from '../ui/permissions.js';
import { checkCooldown } from '../util/rateLimiter.js';
import { BUTTON_COOLDOWN_MS } from '../player/constants.js';
import type { BotCommand } from './types.js';

export const skipCommand: BotCommand = {
  data: new SlashCommandBuilder().setName('skip').setDescription('現在の曲をスキップします'),

  async execute(interaction: ChatInputCommandInteraction): Promise<void> {
    if (!interaction.inCachedGuild()) {
      await interaction.reply({ content: 'サーバー内でのみ使用できます。', flags: MessageFlags.Ephemeral });
      return;
    }

    const player = GuildPlayerManager.get(interaction.guildId);
    if (!player || player.destroyed || !player.currentTrack) {
      await interaction.reply({ content: '再生中の曲がありません。', flags: MessageFlags.Ephemeral });
      return;
    }

    const perm = checkControlPermission(interaction, player);
    if (!perm.ok) {
      await interaction.reply({ content: perm.reason ?? '権限がありません。', flags: MessageFlags.Ephemeral });
      return;
    }

    if (!checkCooldown('skip', interaction.user.id, BUTTON_COOLDOWN_MS)) {
      await interaction.reply({ content: '少し間隔を空けてから再度お試しください。', flags: MessageFlags.Ephemeral });
      return;
    }

    await interaction.deferReply({ flags: MessageFlags.Ephemeral });
    await player.skip();
    await interaction.editReply({ content: 'スキップしました。' });
  },
};
