import {
  MessageFlags,
  PermissionFlagsBits,
  SlashCommandBuilder,
  type ChatInputCommandInteraction,
} from 'discord.js';
import * as GuildPlayerManager from '../player/GuildPlayerManager.js';
import { getGuildSettings, upsertGuildSettings, type PermissionMode } from '../data/guildSettingsRepo.js';
import type { BotCommand } from './types.js';

const PERMISSION_MODE_LABELS: Record<PermissionMode, string> = {
  'same-voice-channel': '同じボイスチャンネルの全員',
  'dj-role': 'DJロール保持者のみ',
  'requester-only': '現在の曲をリクエストした人のみ',
};

export const settingsCommand: BotCommand = {
  data: new SlashCommandBuilder()
    .setName('settings')
    .setDescription('サーバーごとのBot設定（操作権限）を確認・変更します')
    .setDefaultMemberPermissions(PermissionFlagsBits.ManageGuild)
    .addSubcommand((sub) => sub.setName('show').setDescription('現在の操作権限設定を表示します'))
    .addSubcommand((sub) =>
      sub
        .setName('permission')
        .setDescription('誰がBotを操作できるかを設定します')
        .addStringOption((opt) =>
          opt
            .setName('mode')
            .setDescription('操作を許可する対象')
            .setRequired(true)
            .addChoices(
              { name: '同じVCの全員（デフォルト）', value: 'same-voice-channel' },
              { name: 'DJロール保持者のみ', value: 'dj-role' },
              { name: '現在の曲をリクエストした人のみ', value: 'requester-only' },
            ),
        )
        .addRoleOption((opt) =>
          opt.setName('dj_role').setDescription('DJロール（mode=DJロール保持者のみ の場合に使用）').setRequired(false),
        ),
    ),

  async execute(interaction: ChatInputCommandInteraction): Promise<void> {
    if (!interaction.inCachedGuild()) {
      await interaction.reply({ content: 'サーバー内でのみ使用できます。', flags: MessageFlags.Ephemeral });
      return;
    }

    // Server-side re-check: the setDefaultMemberPermissions gate is client-side and per-guild overridable.
    if (!interaction.member.permissions.has(PermissionFlagsBits.ManageGuild)) {
      await interaction.reply({
        content: 'この操作には「サーバー管理」権限が必要です。',
        flags: MessageFlags.Ephemeral,
      });
      return;
    }

    const subcommand = interaction.options.getSubcommand();

    if (subcommand === 'show') {
      const settings = getGuildSettings(interaction.guildId);
      const roleLine =
        settings.permissionMode === 'dj-role'
          ? `\nDJロール: ${settings.djRoleId ? `<@&${settings.djRoleId}>` : '未設定（設定するまで同じVCの全員が操作可能）'}`
          : '';
      await interaction.reply({
        content: `**現在の操作権限設定**\nモード: ${PERMISSION_MODE_LABELS[settings.permissionMode]}${roleLine}`,
        flags: MessageFlags.Ephemeral,
      });
      return;
    }

    // `mode` is constrained to the three valid values by addChoices, so the cast is safe.
    const mode = interaction.options.getString('mode', true) as PermissionMode;
    const djRole = interaction.options.getRole('dj_role');

    const updated = upsertGuildSettings(
      interaction.guildId,
      djRole ? { permissionMode: mode, djRoleId: djRole.id } : { permissionMode: mode },
    );

    // Push onto the running player so it applies immediately, not only after re-create.
    const player = GuildPlayerManager.get(interaction.guildId);
    if (player && !player.destroyed) {
      player.setPermissionSettings(updated.permissionMode, updated.djRoleId);
    }

    const lines = [`操作権限モードを「${PERMISSION_MODE_LABELS[mode]}」に設定しました。`];
    if (mode === 'dj-role') {
      lines.push(
        updated.djRoleId
          ? `DJロール: <@&${updated.djRoleId}>`
          : '⚠️ DJロールが未設定です。`dj_role` を指定するまでは同じVCの全員が操作できます。',
      );
    }
    await interaction.reply({ content: lines.join('\n'), flags: MessageFlags.Ephemeral });
  },
};
