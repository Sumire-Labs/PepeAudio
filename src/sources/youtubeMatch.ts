import { createQueueItem, type QueueItem, type SourceType } from '../player/QueueItem.js';
import { searchYouTube, createYouTubeStreamGetter } from './youtube.js';
import { LruCache } from '../util/lruCache.js';

export class NoMatchFoundError extends Error {}

export interface MatchOnYouTubeParams {
  title: string;
  artist: string;
  requestedBy: string;
  sourceUrl: string;
  sourceType: SourceType;
}

interface CachedMatch {
  videoId: string;
  url: string;
}

/**
 * Caches resolved (title, artist) -> best YouTube video, so the same popular
 * track being played across guilds (or the same track re-queued) doesn't
 * repeat a YouTube search. Only the videoId/url are cached, never a stream
 * URL — those are signed and expire, videoId isn't. Negative results
 * (NoMatchFoundError) are deliberately never cached: a transient search
 * failure shouldn't be frozen in place for the whole TTL.
 */
const MATCH_CACHE_MAX_SIZE = 1_000;
const MATCH_CACHE_TTL_MS = 6 * 60 * 60 * 1_000;
const matchCache = new LruCache<string, CachedMatch>(MATCH_CACHE_MAX_SIZE, MATCH_CACHE_TTL_MS);

async function findBestYouTubeMatch(title: string, artist: string): Promise<CachedMatch> {
  const cacheKey = `${title} ${artist}`.trim().toLowerCase();
  const cached = matchCache.get(cacheKey);
  if (cached) return cached;

  const candidates = await searchYouTube(cacheKey, 5);
  if (candidates.length === 0) {
    throw new NoMatchFoundError(`「${title}」に一致するYouTube動画が見つかりませんでした。`);
  }

  // Prefer official "- Topic" auto-generated audio channels; penalize covers/live/reactions.
  const scored = candidates.map((candidate) => {
    let score = 0;
    if (/\btopic\b/i.test(candidate.author)) score += 2;
    if (/cover|reaction|live|remix/i.test(candidate.title)) score -= 2;
    return { candidate, score };
  });
  scored.sort((a, b) => b.score - a.score);
  const best = scored[0]!.candidate;

  const result: CachedMatch = { videoId: best.videoId, url: best.url };
  matchCache.set(cacheKey, result);
  return result;
}

/**
 * Shared by Spotify and Apple Music (neither has a public streaming API) —
 * resolves title/artist metadata to a playable track by searching YouTube and
 * picking the best candidate. Matches EAGERLY (used for single-track links,
 * where the /play response itself should say "not found" if nothing matches).
 */
export async function matchOnYouTube(params: MatchOnYouTubeParams): Promise<QueueItem> {
  const { title, artist, requestedBy, sourceUrl, sourceType } = params;
  const { videoId, url } = await findBestYouTubeMatch(title, artist);
  return createQueueItem({
    title,
    artist,
    durationMs: null,
    thumbnailUrl: null,
    sourceType,
    sourceUrl,
    requestedBy,
    getStream: createYouTubeStreamGetter(videoId, url),
  });
}

/**
 * Same matching logic as matchOnYouTube, but deferred to the QueueItem's
 * first getStream() call instead of done eagerly — used for playlist/album
 * entries, where matching every track up front would block the /play response
 * on one YouTube search per track. The match result is memoized after the
 * first call so loop/previous/crash-retry replays of the same item don't
 * search again. A match failure surfaces as a getStream() rejection, handled
 * by GuildPlayer's existing playback-failure "skip to next" path exactly like
 * any other stream-resolution error - no new error handling needed there.
 */
export function createLazyMatchedQueueItem(params: MatchOnYouTubeParams): QueueItem {
  const { title, artist, requestedBy, sourceUrl, sourceType } = params;
  let memoized: CachedMatch | null = null;

  const resolveMatch = async (): Promise<CachedMatch> => {
    memoized ??= await findBestYouTubeMatch(title, artist);
    return memoized;
  };

  return createQueueItem({
    title,
    artist,
    durationMs: null,
    thumbnailUrl: null,
    sourceType,
    sourceUrl,
    requestedBy,
    getStream: async () => {
      const { videoId, url } = await resolveMatch();
      return createYouTubeStreamGetter(videoId, url)();
    },
    prefetch: async () => {
      await resolveMatch();
    },
  });
}
