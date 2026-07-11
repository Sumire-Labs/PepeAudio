import type { ChatInputCommandInteraction } from 'discord.js';
import {
  resolveInput,
  SourceResolutionError,
  YouTubeUnavailableError,
  NoMatchFoundError,
  SpotifyResolutionError,
  SoundCloudUnavailableError,
  AppleMusicResolutionError,
} from '../../sources/index.js';
import type { QueueItem } from '../../player/QueueItem.js';
import { logger } from '../../logger.js';

const KNOWN_ERROR_TYPES = [
  SourceResolutionError,
  YouTubeUnavailableError,
  NoMatchFoundError,
  SpotifyResolutionError,
  SoundCloudUnavailableError,
  AppleMusicResolutionError,
];

/**
 * Resolves the /play query into queue items. Handles every recognized
 * resolver error type by editing the reply with that error's own message; an
 * unrecognized error is logged and gets a generic reply instead. Returns the
 * resolved items, or null if it already handled a reply (so the caller knows
 * to stop).
 */
export async function resolvePlayQuery(
  query: string,
  userId: string,
  interaction: ChatInputCommandInteraction,
): Promise<QueueItem[] | null> {
  let items: QueueItem[];
  try {
    items = await resolveInput(query, userId);
  } catch (err) {
    if (KNOWN_ERROR_TYPES.some((ErrorType) => err instanceof ErrorType)) {
      await interaction.editReply({ content: (err as Error).message });
    } else {
      logger.error({ err, query }, 'Unhandled error resolving /play input');
      await interaction.editReply({ content: '再生できませんでした。リンクを確認してください。' });
    }
    return null;
  }

  if (items.length === 0) {
    await interaction.editReply({ content: '再生できる曲が見つかりませんでした。' });
    return null;
  }

  return items;
}
