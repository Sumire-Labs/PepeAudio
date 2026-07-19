/**
 * Web dashboard authorization boundary, keyed on a raw userId instead of an
 * interaction. Keep in lockstep with src/ui/permissions.ts
 * `checkControlPermission` if that logic changes.
 */
import { PermissionFlagsBits, type Guild } from 'discord.js';
import type { GuildPlayer } from '../../player/GuildPlayer.js';
import type { ViewerCapabilities } from './types.js';

function allow(): ViewerCapabilities {
  return { canControl: true, denyReason: null, inBotVoiceChannel: true };
}

export async function resolveViewerCapabilities(
  guild: Guild | undefined,
  userId: string,
  player: GuildPlayer,
): Promise<ViewerCapabilities> {
  if (!guild) {
    return { canControl: false, denyReason: 'サーバーの状態を確認できませんでした。もう一度お試しください。', inBotVoiceChannel: false };
  }

  // Fetch on cache miss so an uncached member isn't wrongly denied; a fetch
  // failure (not a member / left the guild) is a denial.
  let member = guild.members.cache.get(userId);
  if (!member) {
    try {
      member = await guild.members.fetch(userId);
    } catch {
      return { canControl: false, denyReason: 'メンバー情報を取得できませんでした。', inBotVoiceChannel: false };
    }
  }

  // member.voice.channelId requires the GuildVoiceStates intent (index.ts).
  if (member.voice.channelId !== player.voiceChannelId) {
    return { canControl: false, denyReason: '操作するにはBotと同じボイスチャンネルに参加してください。', inBotVoiceChannel: false };
  }

  // Managers/admins bypass the per-mode restriction, but not the same-VC base.
  if (member.permissions.has(PermissionFlagsBits.ManageGuild)) return allow();

  switch (player.permissionMode) {
    case 'dj-role': {
      if (!player.djRoleId) return allow(); // unconfigured dj-role fails OPEN to the same-VC baseline
      if (member.roles.cache.has(player.djRoleId)) return allow();
      return { canControl: false, denyReason: `Botを操作するには <@&${player.djRoleId}> ロールが必要です。`, inBotVoiceChannel: true };
    }
    case 'requester-only': {
      const requesterId = player.currentTrack?.requestedBy;
      if (!requesterId || userId === requesterId) return allow();
      return { canControl: false, denyReason: '現在再生中の曲をリクエストした人のみ操作できます。', inBotVoiceChannel: true };
    }
    case 'same-voice-channel':
    default:
      return allow();
  }
}
