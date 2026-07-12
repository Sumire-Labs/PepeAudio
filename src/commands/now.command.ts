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

    // Relocating the panel reassigns the guild's panel/text channel and re-sends
    // the message for everyone, so require the caller to actually be in the
    // bot's voice channel — otherwise any guild member could repost/move the
    // panel from outside the session. (This is the same base requirement every
    // control action shares; the panel move is intentionally NOT gated by the
    // per-guild dj-role/requester mode so any listener can summon it.)
    if (interaction.member.voice.channelId !== player.voiceChannelId) {
      await interaction.reply({ content: 'パネルを表示するにはBotと同じボイスチャンネルに参加してください。', flags: MessageFlags.Ephemeral });
      return;
    }

    // Reposting the panel sends a fresh message; throttle per-user so it can't be
    // used to spam a channel.
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
