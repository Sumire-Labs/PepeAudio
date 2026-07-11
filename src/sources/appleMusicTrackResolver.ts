import type { QueueItem } from '../player/QueueItem.js';
import { matchOnYouTube } from './youtubeMatch.js';
import { logger } from '../logger.js';
import { lookup, type ItunesResult, AppleMusicResolutionError } from './appleMusicClient.js';

export async function resolveAppleMusicTrack(id: string, requestedBy: string, url: string): Promise<QueueItem[]> {
  let results: ItunesResult[];
  try {
    results = await lookup(id);
  } catch (err) {
    logger.error({ err, url }, 'Apple Music track metadata resolution failed');
    throw new AppleMusicResolutionError('Apple Musicの曲情報の取得に失敗しました。');
  }
  const track = results[0];
  if (!track?.trackName || !track.artistName) {
    throw new AppleMusicResolutionError('Apple Musicの曲情報を読み取れませんでした。');
  }
  return [
    await matchOnYouTube({
      title: track.trackName,
      artist: track.artistName,
      requestedBy,
      sourceUrl: url,
      sourceType: 'applemusic',
    }),
  ];
}
