import type { ChatInputCommandInteraction, VoiceBasedChannel } from 'discord.js';
import * as GuildPlayerManager from '../../player/GuildPlayerManager.js';
import type { GuildPlayer } from '../../player/GuildPlayer.js';
import { getFfmpegCapabilities } from '../../config/ffmpegState.js';
import { logger } from '../../logger.js';

/**
 * Gets (or creates) the guild's player, connected to the voice channel the
 * caller is in. Returns the ready player, or null if it already sent an
 * editReply and the caller should stop.
 */
export async function acquireGuildPlayer(
  interaction: ChatInputCommandInteraction<'cached'>,
  voiceChannel: VoiceBasedChannel,
): Promise<GuildPlayer | null> {
  // Re-checked here (not just before the awaits above) to close a race where
  // a different user's /play could have created a guild player in another
  // voice channel while this command was resolving its input.
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
    // The old channel is abandoned - tear that session down so we can
    // connect to the channel this command was actually run from.
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
    // Without this, the never-ready player stays registered (not destroyed),
    // so GuildPlayerManager.getOrCreate() keeps handing it back on every
    // subsequent /play in this guild - permanently stuck until a manual
    // /stop or /quit (which nothing here suggests) or a process restart.
    await GuildPlayerManager.destroy(interaction.guildId);
    await interaction.editReply({ content: 'ボイスチャンネルへの接続に失敗しました。もう一度お試しください。' });
    return null;
  }

  // A second /play (from a different voice channel) can win the
  // reclaim-an-abandoned-channel race above between this command's destroy()
  // and getOrCreate() calls - if so, this command's caller isn't actually in
  // the channel the (other) player ended up connected to.
  if (player.voiceChannelId !== voiceChannel.id) {
    await interaction.editReply({
      content: `他の操作と競合したため、Botは <#${player.voiceChannelId}> に接続しました。そちらに参加してから再度お試しください。`,
    });
    return null;
  }

  return player;
}
