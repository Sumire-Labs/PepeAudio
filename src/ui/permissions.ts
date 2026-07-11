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
 * The gate for every user-initiated control action (slash commands + panel
 * buttons/selects/modals). Every mode shares one base requirement — you must be
 * in the bot's voice channel — and then layers the per-guild permissionMode on
 * top (set via /settings, persisted in guildSettingsRepo, mirrored onto the
 * live player via GuildPlayer.setPermissionSettings).
 *
 * Members with "Manage Server" bypass the per-mode restriction (but NOT the
 * base same-voice-channel requirement), an escape hatch so a mis-configured
 * dj-role/requester-only can never lock a guild's own managers out of the bot.
 * An unimplemented/unknown mode fails safe to the same-voice-channel baseline
 * rather than either denying everyone or silently granting more than intended.
 */
export function checkControlPermission(interaction: ControllableInteraction, player: GuildPlayer): PermissionCheckResult {
  if (!interaction.inCachedGuild()) {
    return { ok: false, reason: 'サーバーの状態を確認できませんでした。もう一度お試しください。' };
  }

  // Base requirement for every mode: be in the same voice channel as the bot.
  if (interaction.member.voice.channelId !== player.voiceChannelId) {
    return { ok: false, reason: '操作するにはBotと同じボイスチャンネルに参加してください。' };
  }

  // Server managers bypass the additional per-mode restriction below.
  // PermissionsBitField#has honors the Administrator bit, so admins pass too.
  if (interaction.member.permissions.has(PermissionFlagsBits.ManageGuild)) {
    return { ok: true };
  }

  switch (player.permissionMode) {
    case 'dj-role': {
      // dj-role with no role configured yet can't be enforced — fail OPEN to the
      // same-VC baseline rather than deny everyone (admins already passed above).
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
