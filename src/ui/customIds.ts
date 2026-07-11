export const PANEL_ACTIONS = ['prev', 'playpause', 'skip', 'stop', 'shuffle', 'loop', 'spatial', 'volume', 'addQueue'] as const;
export type PanelAction = (typeof PANEL_ACTIONS)[number];

export function buildCustomId(action: PanelAction, guildId: string): string {
  return `panel:${action}:${guildId}`;
}

export interface ParsedCustomId {
  action: PanelAction;
  guildId: string;
}

const ACTION_SET: ReadonlySet<string> = new Set(PANEL_ACTIONS);

export function parseCustomId(customId: string): ParsedCustomId | null {
  const parts = customId.split(':');
  if (parts.length !== 3) return null;
  const [ns, action, guildId] = parts;
  if (ns !== 'panel' || !action || !guildId || !ACTION_SET.has(action)) return null;
  return { action: action as PanelAction, guildId };
}

export interface ParsedAddQueueModalId {
  guildId: string;
}

export function buildAddQueueModalId(guildId: string): string {
  return `panel:addQueueModal:${guildId}`;
}

export function parseAddQueueModalId(customId: string): ParsedAddQueueModalId | null {
  const parts = customId.split(':');
  if (parts.length !== 3 || parts[0] !== 'panel' || parts[1] !== 'addQueueModal' || !parts[2]) return null;
  return { guildId: parts[2] };
}
