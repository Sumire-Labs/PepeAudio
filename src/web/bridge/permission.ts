/**
 * The authorization boundary for the web dashboard. A faithful port of
 * src/ui/permissions.ts `checkControlPermission`, re-expressed against a raw
 * userId (the browser never sends an interaction). Runs ON THE OWNING SHARD for
 * every command — the browser is trusted for nothing beyond the authenticated
 * userId. Keep this in lockstep with permissions.ts if that logic changes.
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

  // Members are cached opportunistically; fetch on a miss so a legitimate user
  // who simply isn't cached yet isn't wrongly denied. A genuine "not a member /
  // left the guild" fetch failure is a denial.
  let member = guild.members.cache.get(userId);
  if (!member) {
    try {
      member = await guild.members.fetch(userId);
    } catch {
      return { canControl: false, denyReason: 'メンバー情報を取得できませんでした。', inBotVoiceChannel: false };
    }
  }

  // Base requirement for every mode: be in the same voice channel as the bot.
  // member.voice.channelId is populated by the GuildVoiceStates intent (index.ts).
  if (member.voice.channelId !== player.voiceChannelId) {
    return { canControl: false, denyReason: '操作するにはBotと同じボイスチャンネルに参加してください。', inBotVoiceChannel: false };
  }

  // Server managers/admins bypass the per-mode restriction (not the same-VC base).
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
