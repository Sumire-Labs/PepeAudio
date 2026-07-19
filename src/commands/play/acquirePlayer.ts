import type { ChatInputCommandInteraction, VoiceBasedChannel } from 'discord.js';
import * as GuildPlayerManager from '../../player/GuildPlayerManager.js';
import type { GuildPlayer } from '../../player/GuildPlayer.js';
import { getFfmpegCapabilities } from '../../config/ffmpegState.js';
import { logger } from '../../logger.js';

/** Returns the ready guild player, or null if it already sent an editReply and the caller must bail. */
export async function acquireGuildPlayer(
  interaction: ChatInputCommandInteraction<'cached'>,
  voiceChannel: VoiceBasedChannel,
): Promise<GuildPlayer | null> {
  // Re-check to close a race: another user's /play could have created a player
  // in a different voice channel while this command resolved its input.
  const existing = GuildPlayerManager.get(interaction.guildId);
  if (existing && !existing.destroyed && existing.voiceChannelId !== voiceChannel.id) {
    const otherChannel = interaction.guild.channels.cache.get(existing.voiceChannelId);
    const nonBotMembers = otherChannel?.isVoiceBased() ? otherChannel.members.filter((m) => !m.user.bot).size : 0;
    if (nonBotMembers > 0) {
      await interaction.editReply({
        content: `既に <#${existing.voiceChannelId}> で再生中です。そちらに参加するか \`/stop\` してください。`,
      });
      return null;
    }
    // Old channel abandoned - tear it down so we can connect where this command ran.
    await GuildPlayerManager.destroy(interaction.guildId);
  }

  const player = GuildPlayerManager.getOrCreate({
    guildId: interaction.guildId,
    textChannelId: interaction.channelId,
    voiceChannelId: voiceChannel.id,
    adapterCreator: interaction.guild.voiceAdapterCreator,
    ffmpeg: getFfmpegCapabilities(),
  });

  try {
    await player.waitUntilReady();
  } catch (err) {
    logger.error({ err }, 'Voice connection failed to become ready');
    // Otherwise the never-ready player stays registered and getOrCreate() keeps
    // handing it back on every /play until a manual /stop, /quit, or restart.
    await GuildPlayerManager.destroy(interaction.guildId);
    await interaction.editReply({ content: 'ボイスチャンネルへの接続に失敗しました。もう一度お試しください。' });
    return null;
  }

  // A second /play can win the reclaim race above between our destroy() and
  // getOrCreate() - if so, the caller isn't in the channel the player connected to.
  if (player.voiceChannelId !== voiceChannel.id) {
    await interaction.editReply({
      content: `他の操作と競合したため、Botは <#${player.voiceChannelId}> に接続しました。そちらに参加してから再度お試しください。`,
    });
    return null;
  }

  return player;
}
