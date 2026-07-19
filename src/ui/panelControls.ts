import {
  ActionRowBuilder,
  ButtonBuilder,
  ButtonStyle,
  type ContainerBuilder,
  StringSelectMenuBuilder,
} from 'discord.js';
import type { GuildPlayer } from '../player/GuildPlayer.js';
import { buildCustomId, type PanelAction } from './customIds.js';
import { AURA_ENABLED, VOLUME_PRESETS } from '../player/constants.js';
import { getHrirProfiles } from '../config/hrirProfilesState.js';
import { loopLabel } from './panelLabels.js';

function btn(action: PanelAction, guildId: string, label: string, style: ButtonStyle, disabled = false): ButtonBuilder {
  return new ButtonBuilder().setCustomId(buildCustomId(action, guildId)).setLabel(label).setStyle(style).setDisabled(disabled);
}

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
    // Persistent per-guild mode: stays enabled with no track (unlike its row-mates).
    btn('autoplay', player.guildId, '📻 オート', player.autoplay ? ButtonStyle.Success : ButtonStyle.Secondary),
  );

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

  const row4 = new ActionRowBuilder<ButtonBuilder>().addComponents(
    btn('addQueue', player.guildId, '➕ 曲を追加', ButtonStyle.Success),
  );
  if (AURA_ENABLED) {
    row4.addComponents(
      btn(
        'hrir',
        player.guildId,
        'Aura HRIR',
        player.hrirMode === 'off' ? ButtonStyle.Secondary : ButtonStyle.Success,
        !track,
      ),
    );
  }

  // Separate calls per row: button and select-menu rows have different generics
  // that a single mixed variadic call can't type-check.
  container.addActionRowComponents(row1);
  container.addActionRowComponents(row2);
  container.addActionRowComponents(row3);
  container.addActionRowComponents(row4);

  // Only shown with >1 profile: a single profile is auto-applied, and this keeps
  // the panel within Discord's 5-row cap.
  const hrirProfiles = getHrirProfiles();
  if (AURA_ENABLED && hrirProfiles.length > 1) {
    // Value stays the exact id (label prettified only) so it round-trips to setAuraPreset.
    const prettyId = (id: string): string => id.replace(/_/g, ' ');
    const presetSelect = new StringSelectMenuBuilder()
      .setCustomId(buildCustomId('preset', player.guildId))
      .setPlaceholder(`Aura プリセット: ${player.hrirProfile ? prettyId(player.hrirProfile) : 'なし'}`)
      .setDisabled(!track || player.hrirMode === 'off')
      .addOptions(
        hrirProfiles.slice(0, 25).map((p) => ({
          label: prettyId(p.id).slice(0, 100),
          value: p.id,
          default: p.id === player.hrirProfile,
        })),
      );
    const row5 = new ActionRowBuilder<StringSelectMenuBuilder>().addComponents(presetSelect);
    container.addActionRowComponents(row5);
  }
}
