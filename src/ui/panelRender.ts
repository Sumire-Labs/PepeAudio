import { MessageFlags, type MessageCreateOptions, type MessageEditOptions } from 'discord.js';
import type { GuildPlayer } from '../player/GuildPlayer.js';
import { buildPanelComponents } from './panelBuilder.js';
import { getFfmpegCaps } from './panelState.js';

export function renderComponents(player: GuildPlayer) {
  return [buildPanelComponents(player, { sofalizerAvailable: getFfmpegCaps().sofalizerAvailable })];
}

export function renderSendOptions(player: GuildPlayer): MessageCreateOptions {
  // Silent: the info section mentions the requester (`<@id>`), which would
  // otherwise ping them (and everyone else with mention notifications on)
  // every time a fresh panel is sent.
  return {
    flags: [MessageFlags.IsComponentsV2, MessageFlags.SuppressNotifications],
    components: renderComponents(player),
  };
}

export function renderEditOptions(player: GuildPlayer): MessageEditOptions {
  return { flags: MessageFlags.IsComponentsV2, components: renderComponents(player) };
}
