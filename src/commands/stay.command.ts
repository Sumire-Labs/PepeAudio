import { MessageFlags, SlashCommandBuilder, type ChatInputCommandInteraction } from 'discord.js';
import * as GuildPlayerManager from '../player/GuildPlayerManager.js';
import { checkControlPermission } from '../ui/permissions.js';
import type { BotCommand } from './types.js';

export const stayCommand: BotCommand = {
  data: new SlashCommandBuilder()
    .setName('stay')
    .setDescription('24/7モード（無人・キューが空でも自動退出しない）の切り替え')
    .addBooleanOption((opt) => opt.setName('enabled').setDescription('有効にする場合はtrue').setRequired(true)),

  async execute(interaction: ChatInputCommandInteraction): Promise<void> {
    if (!interaction.inCachedGuild()) {
      await interaction.reply({ content: 'サーバー内でのみ使用できます。', flags: MessageFlags.Ephemeral });
      return;
    }

    const player = GuildPlayerManager.get(interaction.guildId);
    if (!player || player.destroyed) {
      await interaction.reply({
        content: 'Botはまだボイスチャンネルに参加していません。`/play` で開始してから設定してください。',
        flags: MessageFlags.Ephemeral,
      });
      return;
    }

    const perm = checkControlPermission(interaction, player);
    if (!perm.ok) {
      await interaction.reply({ content: perm.reason ?? '権限がありません。', flags: MessageFlags.Ephemeral });
      return;
    }

    const enabled = interaction.options.getBoolean('enabled', true);
    player.setStay247(enabled);
    await interaction.reply({
      content: enabled
        ? '24/7モードを有効にしました。無人・キューが空でも自動退出しません（`/quit`または`/stop`で手動退出できます）。'
        : '24/7モードを無効にしました。通常通り、無人またはキューが空の状態が続くと自動退出します。',
      flags: MessageFlags.Ephemeral,
    });
  },
};
