import { MessageFlags, SlashCommandBuilder, type ChatInputCommandInteraction } from 'discord.js';
import { runPreflightGuards } from './play/guards.js';
import { resolvePlayQuery } from './play/resolveQuery.js';
import { acquireGuildPlayer } from './play/acquirePlayer.js';
import { enqueueAndConfirm } from './play/enqueueAndConfirm.js';
import * as GuildPlayerManager from '../player/GuildPlayerManager.js';
import { checkControlPermission } from '../ui/permissions.js';
import type { BotCommand } from './types.js';

export const playCommand: BotCommand = {
  data: new SlashCommandBuilder()
    .setName('play')
    .setDescription('Spotify/SoundCloud/YouTube/Apple Musicのリンク、または検索ワードを再生します')
    .addStringOption((opt) =>
      opt.setName('query').setDescription('リンクまたは検索ワード').setRequired(true).setMaxLength(200),
    ),

  async execute(interaction: ChatInputCommandInteraction): Promise<void> {
    const guards = await runPreflightGuards(interaction);
    if (!guards) return;
    const { interaction: cachedInteraction, voiceChannel } = guards;

    // Adding to the queue is a control action, so it must obey the guild's
    // permissionMode exactly like the panel's add-queue button does (see
    // addQueueModalHandler). Gate only when a live session ALREADY exists and
    // the caller is in the bot's channel — the same situation the panel covers.
    // A brand-new session (no player yet, or the caller starting playback in a
    // fresh/abandoned channel) can't be gated against a player that doesn't
    // exist, and acquireGuildPlayer still arbitrates the cross-channel cases.
    const existing = GuildPlayerManager.get(cachedInteraction.guildId);
    if (existing && !existing.destroyed && cachedInteraction.member.voice.channelId === existing.voiceChannelId) {
      const perm = checkControlPermission(cachedInteraction, existing);
      if (!perm.ok) {
        await cachedInteraction.reply({ content: perm.reason ?? '権限がありません。', flags: MessageFlags.Ephemeral });
        return;
      }
    }

    await cachedInteraction.deferReply({ flags: MessageFlags.Ephemeral });

    const query = cachedInteraction.options.getString('query', true);

    const items = await resolvePlayQuery(query, cachedInteraction.user.id, cachedInteraction);
    if (!items) return;

    const player = await acquireGuildPlayer(cachedInteraction, voiceChannel);
    if (!player) return;

    await enqueueAndConfirm(cachedInteraction, player, items);
  },
};
