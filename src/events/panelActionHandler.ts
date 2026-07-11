import {
  MessageFlags,
  type ButtonInteraction,
  type StringSelectMenuInteraction,
} from 'discord.js';
import { parseCustomId } from '../ui/customIds.js';
import { buildAddQueueModal } from '../ui/addQueueModal.js';
import * as GuildPlayerManager from '../player/GuildPlayerManager.js';
import { checkControlPermission } from '../ui/permissions.js';
import { checkCooldown } from '../util/rateLimiter.js';
import { BUTTON_COOLDOWN_MS, SPATIAL_AUDIO_ENABLED, VOLUME_COOLDOWN_MS, type LoopMode } from '../player/constants.js';
import { logger } from '../logger.js';

export async function handleButtonOrSelect(interaction: ButtonInteraction | StringSelectMenuInteraction): Promise<void> {
  const parsed = parseCustomId(interaction.customId);
  if (!parsed) return; // not one of ours

  if (!interaction.inCachedGuild() || interaction.guildId !== parsed.guildId) {
    await interaction.reply({ content: '不正な操作です。', flags: MessageFlags.Ephemeral });
    return;
  }

  const player = GuildPlayerManager.get(parsed.guildId);
  if (!player || player.destroyed) {
    await interaction.reply({ content: 'このパネルは無効です。`/now` で再表示してください。', flags: MessageFlags.Ephemeral });
    return;
  }

  // The crux of "only the newest panel works": any interaction on a superseded message is rejected here.
  if (interaction.message.id !== player.panelMessageId) {
    await interaction.reply({ content: 'このパネルは古くなっています。最新のパネルをご利用ください。', flags: MessageFlags.Ephemeral });
    return;
  }

  const perm = checkControlPermission(interaction, player);
  if (!perm.ok) {
    await interaction.reply({ content: perm.reason ?? '権限がありません。', flags: MessageFlags.Ephemeral });
    return;
  }

  const cooldownMs = parsed.action === 'volume' ? VOLUME_COOLDOWN_MS : BUTTON_COOLDOWN_MS;
  if (!checkCooldown(`panel:${parsed.action}`, interaction.user.id, cooldownMs)) {
    await interaction.reply({ content: '少し間隔を空けてください。', flags: MessageFlags.Ephemeral });
    return;
  }

  if (parsed.action === 'addQueue') {
    // showModal() must be the interaction's first/only response - it cannot follow deferUpdate() below, unlike every other action.
    await interaction.showModal(buildAddQueueModal(parsed.guildId));
    return;
  }

  await interaction.deferUpdate();

  try {
    switch (parsed.action) {
      case 'prev': {
        const result = await player.previous();
        if (!result.ok) {
          const message = result.reason === 'no-history' ? '前の曲はありません。' : '前の曲の再生に失敗しました。';
          await interaction.followUp({ content: message, flags: MessageFlags.Ephemeral });
        }
        break;
      }
      case 'playpause':
        if (player.isPaused()) await player.resume();
        else await player.pause();
        break;
      case 'skip':
        await player.skip();
        break;
      case 'stop':
        await player.stop();
        break;
      case 'shuffle':
        player.toggleShuffle();
        break;
      case 'loop': {
        const order: LoopMode[] = ['off', 'track', 'queue'];
        const next = order[(order.indexOf(player.loopMode) + 1) % order.length]!;
        player.setLoopMode(next);
        break;
      }
      case 'spatial':
        // Button is hidden on freshly-sent panels while this is disabled; this guard
        // only matters for a stale pre-update panel message still showing the button.
        if (!SPATIAL_AUDIO_ENABLED) {
          await interaction.followUp({ content: '3Dオーディオは音質改善のため現在一時的に無効化されています。', flags: MessageFlags.Ephemeral });
          break;
        }
        await player.setSpatialMode(player.spatialMode === 'off' ? 'on' : 'off');
        break;
      case 'volume': {
        if (interaction.isStringSelectMenu()) {
          const value = Number(interaction.values[0]);
          if (!Number.isNaN(value)) player.setVolume(value);
        }
        break;
      }
    }
  } catch (err) {
    logger.error({ err, action: parsed.action, guildId: parsed.guildId }, 'Panel action handler failed');
    try {
      await interaction.followUp({ content: '操作に失敗しました。もう一度お試しください。', flags: MessageFlags.Ephemeral });
    } catch (followUpErr) {
      logger.error({ err: followUpErr }, 'Failed to send panel action failure notice');
    }
  }
}
