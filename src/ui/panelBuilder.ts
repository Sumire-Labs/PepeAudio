import { ContainerBuilder, SeparatorBuilder } from 'discord.js';
import type { GuildPlayer } from '../player/GuildPlayer.js';
import { addHrirFooter, addNowPlayingSection, type PanelBuildOptions } from './panelNowPlaying.js';
import { addControlRows } from './panelControls.js';
import { buildQueuedConfirmation } from './panelQueuedConfirmation.js';

export type { PanelBuildOptions };
export { buildQueuedConfirmation };

/** Builds the full Components V2 tree for the player panel. Container accent_color is intentionally never set. */
export function buildPanelComponents(player: GuildPlayer, opts: PanelBuildOptions): ContainerBuilder {
  const container = new ContainerBuilder();

  addNowPlayingSection(container, player, opts);

  container.addSeparatorComponents(new SeparatorBuilder());

  addControlRows(container, player);
  addHrirFooter(container, player);

  return container;
}
