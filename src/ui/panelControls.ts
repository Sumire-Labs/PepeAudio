import {
  ActionRowBuilder,
  ButtonBuilder,
  ButtonStyle,
  type ContainerBuilder,
  StringSelectMenuBuilder,
} from 'discord.js';
import type { GuildPlayer } from '../player/GuildPlayer.js';
import { buildCustomId, type PanelAction } from './customIds.js';
import { SPATIAL_AUDIO_ENABLED, VOLUME_PRESETS } from '../player/constants.js';
import { loopLabel } from './panelLabels.js';

function btn(action: PanelAction, guildId: string, label: string, style: ButtonStyle, disabled = false): ButtonBuilder {
  return new ButtonBuilder().setCustomId(buildCustomId(action, guildId)).setLabel(label).setStyle(style).setDisabled(disabled);
}

/** Builds and adds the control rows to `container`: row1 (prev/playpause/skip),
 * row2 (stop/shuffle/loop), the conditional row2b (spatial toggle), and row3
 * (the volume select menu). */
export function addControlRows(container: ContainerBuilder, player: GuildPlayer): void {
  const track = player.currentTrack;

  const row1 = new ActionRowBuilder<ButtonBuilder>().addComponents(
    btn('prev', player.guildId, '⏮', ButtonStyle.Secondary, player.history.length === 0),
    btn('playpause', player.guildId, player.isPaused() ? '▶' : '⏸', ButtonStyle.Primary, !track),
    btn('skip', player.guildId, '⏭', ButtonStyle.Secondary, !track),
  );

  const row2 = new ActionRowBuilder<ButtonBuilder>().addComponents(
    btn('stop', player.guildId, '⏹', ButtonStyle.Danger, !track),
    btn('shuffle', player.guildId, '🔀', player.shuffleEnabled ? ButtonStyle.Success : ButtonStyle.Secondary, !track),
    btn(
      'loop',
      player.guildId,
      `🔁 ${loopLabel(player.loopMode)}`,
      player.loopMode === 'off' ? ButtonStyle.Secondary : ButtonStyle.Success,
      !track,
    ),
  );

  // Dormant while SPATIAL_AUDIO_ENABLED is false (see constants.ts) — kept as its
  // own row so re-enabling the flag doesn't require re-adding UI from scratch.
  const row2b = SPATIAL_AUDIO_ENABLED
    ? new ActionRowBuilder<ButtonBuilder>().addComponents(
        btn(
          'spatial',
          player.guildId,
          '🌌 3D Audio(非推奨)',
          player.spatialMode === 'off' ? ButtonStyle.Secondary : ButtonStyle.Success,
          !track,
        ),
      )
    : null;

  const volumeSelect = new StringSelectMenuBuilder()
    .setCustomId(buildCustomId('volume', player.guildId))
    .setPlaceholder(`音量: ${player.volume}%`)
    .setDisabled(!track)
    .addOptions(
      VOLUME_PRESETS.map((v) => ({
        label: v === 0 ? 'ミュート' : `${v}%`,
        value: String(v),
        default: v === player.volume,
      })),
    );
  const row3 = new ActionRowBuilder<StringSelectMenuBuilder>().addComponents(volumeSelect);

  // Called separately per row (rather than one variadic call) since the button
  // rows and row3 hold different component generics (Button vs StringSelectMenu),
  // which a single mixed rest-args call can't type-check against.
  container.addActionRowComponents(row1);
  container.addActionRowComponents(row2);
  if (row2b) container.addActionRowComponents(row2b);
  container.addActionRowComponents(row3);
}
