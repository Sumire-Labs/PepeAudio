import type { QueueItem } from '../player/QueueItem.js';
import { parseAppleMusicUrl } from './appleMusicUrlParser.js';
import { resolveAppleMusicTrack } from './appleMusicTrackResolver.js';
import { resolveAppleMusicAlbum } from './appleMusicAlbumResolver.js';
import { AppleMusicResolutionError } from './appleMusicClient.js';

export { NoMatchFoundError } from './youtubeMatch.js';
export { AppleMusicResolutionError } from './appleMusicClient.js';

export async function resolveAppleMusicUrl(url: string, requestedBy: string): Promise<QueueItem[]> {
  const parsed = parseAppleMusicUrl(url);
  if (parsed.kind === 'unsupported') {
    throw new AppleMusicResolutionError(
      'このApple Musicリンクの形式には対応していません（プレイリストは非対応です）。曲またはアルバムのリンクをご利用ください。',
    );
  }

  if (parsed.kind === 'track') {
    return resolveAppleMusicTrack(parsed.id, requestedBy, url);
  }

  return resolveAppleMusicAlbum(parsed.id, requestedBy, url);
}
