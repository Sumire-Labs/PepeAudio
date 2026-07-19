import {
  PermissionFlagsBits,
  type ButtonInteraction,
  type ChatInputCommandInteraction,
  type ModalSubmitInteraction,
  type StringSelectMenuInteraction,
} from 'discord.js';
import type { GuildPlayer } from '../player/GuildPlayer.js';

export interface PermissionCheckResult {
  ok: boolean;
  reason?: string;
}

type ControllableInteraction =
  | ButtonInteraction
  | StringSelectMenuInteraction
  | ChatInputCommandInteraction
  | ModalSubmitInteraction;

/**
 * Managers (Manage Server) bypass the per-mode restriction but NOT the base
 * same-voice-channel requirement, so a mis-configured mode can't lock them out.
 * Unknown modes fail safe to the same-VC baseline.
 */
export function checkControlPermission(interaction: ControllableInteraction, player: GuildPlayer): PermissionCheckResult {
  if (!interaction.inCachedGuild()) {
    return { ok: false, reason: 'サーバーの状態を確認できませんでした。もう一度お試しください。' };
  }

  // Base requirement for every mode: same voice channel as the bot.
  if (interaction.member.voice.channelId !== player.voiceChannelId) {
    return { ok: false, reason: '操作するにはBotと同じボイスチャンネルに参加してください。' };
  }

  // #has honors the Administrator bit, so admins bypass the per-mode gate too.
  if (interaction.member.permissions.has(PermissionFlagsBits.ManageGuild)) {
    return { ok: true };
  }

  switch (player.permissionMode) {
    case 'dj-role': {
      // No DJ role configured yet → fail OPEN to same-VC baseline, don't deny everyone.
      if (!player.djRoleId) return { ok: true };
      if (interaction.member.roles.cache.has(player.djRoleId)) return { ok: true };
      return { ok: false, reason: `Botを操作するには <@&${player.djRoleId}> ロールが必要です。` };
    }
    case 'requester-only': {
      const requesterId = player.currentTrack?.requestedBy;
      // Nothing playing → no requester to gate on; fall back to the same-VC baseline.
      if (!requesterId || interaction.user.id === requesterId) return { ok: true };
      return { ok: false, reason: '現在再生中の曲をリクエストした人のみ操作できます。' };
    }
    case 'same-voice-channel':
    default:
      return { ok: true };
  }
}
