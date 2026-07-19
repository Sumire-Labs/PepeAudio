import { MessageFlags, type ChatInputCommandInteraction, type VoiceBasedChannel } from 'discord.js';
import { checkCooldown } from '../../util/rateLimiter.js';
import { PLAY_COOLDOWN_MS } from '../../player/constants.js';

export interface PreflightPassResult {
  interaction: ChatInputCommandInteraction<'cached'>;
  voiceChannel: VoiceBasedChannel;
}

// inCachedGuild() narrowing doesn't cross the call boundary; callers must use
// the returned narrowed interaction, not re-narrow the original.
export async function runPreflightGuards(
  interaction: ChatInputCommandInteraction,
): Promise<PreflightPassResult | null> {
  if (!interaction.inCachedGuild()) {
    await interaction.reply({ content: 'サーバー内でのみ使用できます。', flags: MessageFlags.Ephemeral });
    return null;
  }

  const voiceChannel = interaction.member.voice.channel;
  if (!voiceChannel) {
    await interaction.reply({ content: 'まずボイスチャンネルに参加してください。', flags: MessageFlags.Ephemeral });
    return null;
  }

  const me = interaction.guild.members.me;
  const botPermissions = me ? voiceChannel.permissionsFor(me) : null;
  if (!botPermissions?.has(['Connect', 'Speak'])) {
    await interaction.reply({ content: 'そのボイスチャンネルに参加/発言する権限がありません。', flags: MessageFlags.Ephemeral });
    return null;
  }

  if (!checkCooldown('play', interaction.user.id, PLAY_COOLDOWN_MS)) {
    await interaction.reply({ content: '少し間隔を空けてから再度お試しください。', flags: MessageFlags.Ephemeral });
    return null;
  }

  return { interaction, voiceChannel };
}
