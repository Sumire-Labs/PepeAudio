import { MessageFlags, SlashCommandBuilder, type ChatInputCommandInteraction } from 'discord.js';
import { runPreflightGuards } from './play/guards.js';
import { resolvePlayQuery } from './play/resolveQuery.js';
import { acquireGuildPlayer } from './play/acquirePlayer.js';
import { enqueueAndConfirm } from './play/enqueueAndConfirm.js';
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

    await cachedInteraction.deferReply({ flags: MessageFlags.Ephemeral });

    const query = cachedInteraction.options.getString('query', true);

    const items = await resolvePlayQuery(query, cachedInteraction.user.id, cachedInteraction);
    if (!items) return;

    const player = await acquireGuildPlayer(cachedInteraction, voiceChannel);
    if (!player) return;

    await enqueueAndConfirm(cachedInteraction, player, items);
  },
};
