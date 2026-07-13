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

/** Builds and adds the control rows to `container`: row1 (prev/playpause/skip),
 * row2 (stop/shuffle/loop), row3 (the volume select menu), row4 (add-to-queue
 * plus the Aura HRIR / Aura 360° toggles, when AURA_ENABLED), and an optional
 * row5 (the Aura Preset select menu, only when >1 HRIR profile is loaded). */
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
    // Autoplay ("radio") is a persistent per-guild mode, not tied to the current
    // track, so it stays enabled even with nothing playing (unlike its row-mates).
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

  // Row placed last: add-to-queue plus the audio-effect toggles. Add-to-queue is
  // styled/colored apart from the playback-control cluster above
  // (ButtonStyle.Success + ➕), per explicit request for visual separation, and is
  // never disabled - unlike the transport buttons, adding to the queue doesn't
  // require a track to already be current. Aura HRIR (HRIR out-of-head) and
  // Aura 360° (widening + bass) sit alongside it here; they are independent
  // toggles, green when on and grey when off.
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
      btn(
        'aura360',
        player.guildId,
        'Aura 360°',
        player.aura360Mode === 'off' ? ButtonStyle.Secondary : ButtonStyle.Success,
        !track,
      ),
    );
  }

  // Called separately per row (rather than one variadic call) since the button
  // rows and the select-menu rows hold different component generics (Button vs
  // StringSelectMenu), which a single mixed rest-args call can't type-check against.
  container.addActionRowComponents(row1);
  container.addActionRowComponents(row2);
  container.addActionRowComponents(row3);
  container.addActionRowComponents(row4);

  // Optional row5: the Aura Preset selector — picks which BRIR/HRIR impulse
  // response Aura HRIR convolves with. Only rendered when the feature is enabled
  // and more than one profile is loaded (a single profile is applied
  // automatically — nothing to choose), keeping the panel at Discord's 5-row cap.
  // Disabled unless Aura HRIR is on with a track, so it visibly ties to the
  // active effect (the choice still persists per guild via setAuraPreset).
  const hrirProfiles = getHrirProfiles();
  if (AURA_ENABLED && hrirProfiles.length > 1) {
    // Labels prettify the profile id (filename) for display only — underscores →
    // spaces; the option value stays the exact id so it round-trips to setAuraPreset.
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
