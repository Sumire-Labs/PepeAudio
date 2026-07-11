import {
  ActionRowBuilder,
  ButtonBuilder,
  ButtonStyle,
  type ContainerBuilder,
  StringSelectMenuBuilder,
} from 'discord.js';
import type { GuildPlayer } from '../player/GuildPlayer.js';
import { buildCustomId, type PanelAction } from './customIds.js';
import { VOLUME_PRESETS } from '../player/constants.js';
import { loopLabel } from './panelLabels.js';

function btn(action: PanelAction, guildId: string, label: string, style: ButtonStyle, disabled = false): ButtonBuilder {
  return new ButtonBuilder().setCustomId(buildCustomId(action, guildId)).setLabel(label).setStyle(style).setDisabled(disabled);
}

/** Builds and adds the control rows to `container`: row1 (prev/playpause/skip),
 * row2 (stop/shuffle/loop), row3 (the volume select menu), and row4 (add-to-queue).
 * There is no spatial toggle row — 360° Sound is always on. */
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

  // 360° Sound is always on with no user toggle, so there is no spatial button row.

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

  // Deliberately its own row placed last, and styled/colored apart from the
  // playback-control cluster above (ButtonStyle.Success + ➕), per explicit
  // request for visual separation. Never disabled - unlike the transport
  // buttons, adding to the queue doesn't require a track to already be current.
  const row4 = new ActionRowBuilder<ButtonBuilder>().addComponents(
    btn('addQueue', player.guildId, '➕ 曲を追加', ButtonStyle.Success),
  );

  // Called separately per row (rather than one variadic call) since the button
  // rows and row3 hold different component generics (Button vs StringSelectMenu),
  // which a single mixed rest-args call can't type-check against.
  container.addActionRowComponents(row1);
  container.addActionRowComponents(row2);
  container.addActionRowComponents(row3);
  container.addActionRowComponents(row4);
}
