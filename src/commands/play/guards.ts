import { MessageFlags, type ChatInputCommandInteraction, type VoiceBasedChannel } from 'discord.js';
import { checkCooldown } from '../../util/rateLimiter.js';
import { PLAY_COOLDOWN_MS } from '../../player/constants.js';

export interface PreflightPassResult {
  interaction: ChatInputCommandInteraction<'cached'>;
  voiceChannel: VoiceBasedChannel;
}

/**
 * Runs the guard checks every /play invocation must pass before doing any
 * real work. On failure it sends the appropriate ephemeral reply itself and
 * returns null. On success it returns the interaction narrowed by
 * `inCachedGuild()` — that type-predicate narrowing does not cross a
 * function-call boundary, so callers must use this returned, already-narrowed
 * reference rather than trying to re-narrow the original interaction.
 */
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
