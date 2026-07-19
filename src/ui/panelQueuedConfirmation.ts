import { ContainerBuilder, TextDisplayBuilder } from 'discord.js';
import type { QueueItem } from '../player/QueueItem.js';
import { mdLink } from './panelMarkdown.js';

// Deliberately NOT the full panel: replacing the live panel on every queue
// addition would be disruptive when playback is unchanged. accent_color left
// unset to match the main panel.
export function buildQueuedConfirmation(items: QueueItem[]): ContainerBuilder {
  const container = new ContainerBuilder();
  if (items.length === 1) {
    const item = items[0]!;
    container.addTextDisplayComponents(
      new TextDisplayBuilder().setContent(
        `✅ **キューに追加しました**\n${mdLink(item.title, item.sourceUrl)} - ${mdLink(item.artist, item.sourceUrl)}`,
      ),
    );
  } else {
    container.addTextDisplayComponents(
      new TextDisplayBuilder().setContent(`✅ **${items.length}曲をキューに追加しました**`),
    );
  }
  return container;
}
