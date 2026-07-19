import type { Readable } from 'node:stream';
import scdl from '@vncsprd/soundcloud-downloader';
import { createQueueItem, type QueueItem } from '../player/QueueItem.js';
import { parseSoundCloudTimestamp } from '../util/timestamp.js';
import { isSoundCloudHost } from '../util/urlPatterns.js';
import { logger } from '../logger.js';

export class SoundCloudUnavailableError extends Error {}

/**
 * Re-validate the host and rebuild a clean https URL before handing it to scdl
 * (which fetches whatever URL it's given) — don't trust the caller's routing.
 * Drops credentials, port, and fragment but KEEPS the query: SoundCloud
 * private/unlisted shares require a `?secret_token=`.
 */
function sanitizeSoundCloudUrl(rawUrl: string): string {
  let parsed: URL;
  try {
    parsed = new URL(rawUrl);
  } catch {
    throw new SoundCloudUnavailableError('SoundCloudのURLを認識できませんでした。');
  }
  if (!isSoundCloudHost(parsed.hostname.toLowerCase())) {
    throw new SoundCloudUnavailableError('SoundCloudのURLではありません。');
  }
  return `https://${parsed.hostname}${parsed.pathname}${parsed.search}`;
}

interface ScdlTrackInfo {
  title?: string;
  duration?: number; // ms
  user?: { username?: string };
  artwork_url?: string;
}

/**
 * scdl caches its scraped client_id forever; a stale one (SoundCloud rotates
 * them without notice) never self-heals and there's no public refresh API, so
 * clear the (TypeScript-only-private) cached field directly.
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
  // Parse the `#t=` fragment from the ORIGINAL url before sanitize strips it.
  const initialOffsetMs = parseSoundCloudTimestamp(url);
  const cleanUrl = sanitizeSoundCloudUrl(url);

  let info: ScdlTrackInfo;
  try {
    info = (await withClientIdRetry(() => scdl.getInfo(cleanUrl))) as ScdlTrackInfo;
  } catch (err) {
    logger.error({ err, url: cleanUrl }, 'SoundCloud metadata resolution failed');
    throw new SoundCloudUnavailableError(
      'SoundCloudの情報取得に失敗しました。リンクが正しいか確認するか、しばらくしてから再度お試しください。',
    );
  }

  return [
    createQueueItem({
      title: info.title ?? 'Unknown title',
      artist: info.user?.username ?? 'Unknown artist',
      durationMs: typeof info.duration === 'number' && info.duration > 0 ? info.duration : null,
      thumbnailUrl: info.artwork_url ?? null,
      sourceType: 'soundcloud',
      sourceUrl: cleanUrl,
      requestedBy,
      initialOffsetMs,
      getStream: async () => {
        try {
          const stream = (await withClientIdRetry(() => scdl.download(cleanUrl))) as unknown as Readable;
          return { stream };
        } catch (err) {
          logger.error({ err, url: cleanUrl }, 'SoundCloud stream resolution failed');
          throw new SoundCloudUnavailableError('SoundCloudのストリーム取得に失敗しました。');
        }
      },
    }),
  ];
}
