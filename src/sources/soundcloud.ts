import type { Readable } from 'node:stream';
import scdl from '@vncsprd/soundcloud-downloader';
import { createQueueItem, type QueueItem } from '../player/QueueItem.js';
import { parseSoundCloudTimestamp } from '../util/timestamp.js';
import { logger } from '../logger.js';

export class SoundCloudUnavailableError extends Error {}

interface ScdlTrackInfo {
  title?: string;
  duration?: number; // ms
  user?: { username?: string };
  artwork_url?: string;
}

/**
 * @vncsprd/soundcloud-downloader caches its scraped client_id forever once
 * set — confirmed by reading its compiled source: setClientID() with no
 * argument is a no-op if a client_id is already cached, so a stale/rotated
 * client_id (SoundCloud rotates these without notice) never self-heals no
 * matter how many 401s come back. There's no public API to force a refresh,
 * so this reaches into the (TypeScript-only-private) cached field directly.
 */
function forceClientIdRefresh(): void {
  (scdl as unknown as { _clientID?: string })._clientID = undefined;
}

function isUnauthorized(err: unknown): boolean {
  const status =
    (err as { response?: { status?: number } })?.response?.status ?? (err as { status?: number })?.status;
  return status === 401;
}

async function withClientIdRetry<T>(fn: () => Promise<T>): Promise<T> {
  try {
    return await fn();
  } catch (err) {
    if (!isUnauthorized(err)) throw err;
    logger.warn('SoundCloud client_id appears stale (401) - forcing a refresh and retrying once');
    forceClientIdRefresh();
    return fn();
  }
}

export async function resolveSoundCloudUrl(url: string, requestedBy: string): Promise<QueueItem[]> {
  let info: ScdlTrackInfo;
  try {
    info = (await withClientIdRetry(() => scdl.getInfo(url))) as ScdlTrackInfo;
  } catch (err) {
    logger.error({ err, url }, 'SoundCloud metadata resolution failed');
    throw new SoundCloudUnavailableError(
      'SoundCloudの情報取得に失敗しました。リンクが正しいか確認するか、しばらくしてから再度お試しください。',
    );
  }

  return [
    createQueueItem({
      title: info.title ?? 'Unknown title',
      artist: info.user?.username ?? 'Unknown artist',
      // Per QueueItem's contract, durationMs is null for live/unknown-duration content.
      durationMs: typeof info.duration === 'number' && info.duration > 0 ? info.duration : null,
      thumbnailUrl: info.artwork_url ?? null,
      sourceType: 'soundcloud',
      sourceUrl: url,
      requestedBy,
      initialOffsetMs: parseSoundCloudTimestamp(url),
      getStream: async () => {
        try {
          const stream = (await withClientIdRetry(() => scdl.download(url))) as unknown as Readable;
          return { stream };
        } catch (err) {
          logger.error({ err, url }, 'SoundCloud stream resolution failed');
          throw new SoundCloudUnavailableError('SoundCloudのストリーム取得に失敗しました。');
        }
      },
    }),
  ];
}
