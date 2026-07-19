import type { RepliableInteraction } from 'discord.js';
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

/** Returns null if it already sent an error reply (caller should stop). */
export async function resolvePlayQuery(
  query: string,
  userId: string,
  interaction: RepliableInteraction,
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
