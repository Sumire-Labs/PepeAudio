import { MessageFlags, SlashCommandBuilder, type ChatInputCommandInteraction } from 'discord.js';
import * as GuildPlayerManager from '../player/GuildPlayerManager.js';
import { sendOrReplacePanel } from '../ui/panelManager.js';
import { checkCooldown } from '../util/rateLimiter.js';
import { PLAY_COOLDOWN_MS } from '../player/constants.js';
import { logger } from '../logger.js';
import type { BotCommand } from './types.js';

export const nowCommand: BotCommand = {
  data: new SlashCommandBuilder().setName('now').setDescription('現在の再生パネルをこのチャンネルに表示します'),

  async execute(interaction: ChatInputCommandInteraction): Promise<void> {
    if (!interaction.inCachedGuild()) {
      await interaction.reply({ content: 'サーバー内でのみ使用できます。', flags: MessageFlags.Ephemeral });
      return;
    }

    const player = GuildPlayerManager.get(interaction.guildId);
    if (!player || player.destroyed) {
      await interaction.reply({ content: '現在再生中の曲はありません。`/play` で開始してください。', flags: MessageFlags.Ephemeral });
      return;
    }

    // Relocating reassigns the guild's panel/text channel and re-sends for everyone,
    // so gate on VC presence to stop outsiders moving the panel. Intentionally NOT
    // gated by dj-role/requester mode — any listener may summon it.
    if (interaction.member.voice.channelId !== player.voiceChannelId) {
      await interaction.reply({ content: 'パネルを表示するにはBotと同じボイスチャンネルに参加してください。', flags: MessageFlags.Ephemeral });
      return;
    }

    // Fresh message each repost — throttle per-user to prevent channel spam.
    if (!checkCooldown('now', interaction.user.id, PLAY_COOLDOWN_MS)) {
      await interaction.reply({ content: '少し間隔を空けてから再度お試しください。', flags: MessageFlags.Ephemeral });
      return;
    }

    await interaction.deferReply({ flags: MessageFlags.Ephemeral });

    const channel = interaction.channel;
    if (!channel || !('send' in channel)) {
      await interaction.editReply({ content: 'このチャンネルにはパネルを表示できません。' });
      return;
    }

    try {
      await sendOrReplacePanel(player, channel);
    } catch (err) {
      logger.error({ err }, 'Failed to send/replace the player panel');
      await interaction.editReply({ content: 'パネルの表示に失敗しました。' });
      return;
    }
    await interaction.editReply({ content: 'パネルを表示しました。' });
  },
};
