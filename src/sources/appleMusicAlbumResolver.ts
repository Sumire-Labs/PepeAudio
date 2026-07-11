import type { QueueItem } from '../player/QueueItem.js';
import { createLazyMatchedQueueItem } from './youtubeMatch.js';
import { MAX_PLAYLIST_TRACKS } from '../player/constants.js';
import { logger } from '../logger.js';
import { lookup, type ItunesResult, AppleMusicResolutionError } from './appleMusicClient.js';

export async function resolveAppleMusicAlbum(id: string, requestedBy: string, url: string): Promise<QueueItem[]> {
  // Album: entity=song returns the collection itself (wrapperType: 'collection')
  // followed by one entry per track (wrapperType: 'track').
  let results: ItunesResult[];
  try {
    results = await lookup(id, { entity: 'song' });
  } catch (err) {
    logger.error({ err, url }, 'Apple Music album metadata resolution failed');
    throw new AppleMusicResolutionError('Apple Musicのアルバム情報の取得に失敗しました。');
  }
  const allTracks = results.filter((r) => r.wrapperType === 'track');
  // Cap resolution work per request — mirrors Spotify's playlist/album cap (see MAX_PLAYLIST_TRACKS).
  const tracks = allTracks.slice(0, MAX_PLAYLIST_TRACKS);
  if (allTracks.length > tracks.length) {
    logger.info(
      { url, total: allTracks.length, cap: MAX_PLAYLIST_TRACKS },
      'Apple Music album exceeds the per-request track cap - resolving only the first tracks',
    );
  }
  if (tracks.length === 0) {
    throw new AppleMusicResolutionError('Apple Musicのアルバム内の曲情報を取得できませんでした。');
  }

  // YouTube matching is deferred to each item's first getStream() call (see
  // createLazyMatchedQueueItem) rather than done eagerly here - see the same
  // rationale in spotify.ts's playlist/album branch.
  const items: QueueItem[] = [];
  let failed = 0;
  for (const track of tracks) {
    if (!track.trackName || !track.artistName) {
      failed += 1;
      continue;
    }
    items.push(
      createLazyMatchedQueueItem({
        title: track.trackName,
        artist: track.artistName,
        requestedBy,
        sourceUrl: url,
        sourceType: 'applemusic',
      }),
    );
  }
  if (items.length === 0) {
    throw new AppleMusicResolutionError('アルバム内の曲情報を1つも読み取れませんでした。');
  }
  if (failed > 0) {
    logger.info({ parsed: items.length, failed }, 'Apple Music album partially parsed (YouTube matching happens lazily per track)');
  }
  return items;
}
