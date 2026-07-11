import type { ButtonInteraction, ChatInputCommandInteraction, StringSelectMenuInteraction } from 'discord.js';
import type { GuildPlayer } from '../player/GuildPlayer.js';

export interface PermissionCheckResult {
  ok: boolean;
  reason?: string;
}

type ControllableInteraction = ButtonInteraction | StringSelectMenuInteraction | ChatInputCommandInteraction;

/**
 * Phase 1 hardcodes 'same-voice-channel'. player.permissionMode is read (not
 * ignored) so a future 'dj-role'/'requester-only' branch is a pure addition here,
 * not a call-site change everywhere permissions are checked.
 */
export function checkControlPermission(interaction: ControllableInteraction, player: GuildPlayer): PermissionCheckResult {
  if (!interaction.inCachedGuild()) {
    return { ok: false, reason: 'サーバーの状態を確認できませんでした。もう一度お試しください。' };
  }

  const voiceChannelId = interaction.member.voice.channelId;
  if (voiceChannelId !== player.voiceChannelId) {
    return { ok: false, reason: '操作するにはBotと同じボイスチャンネルに参加してください。' };
  }
  return { ok: true };
}
