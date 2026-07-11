import { createQueueItem, type QueueItem, type SourceType } from '../player/QueueItem.js';
import { searchYouTube, createYouTubeStreamGetter, fetchYouTubeMetadata } from './youtube.js';
import { LruCache } from '../util/lruCache.js';
import { logger } from '../logger.js';

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
  durationMs: number | null;
  thumbnailUrl: string | null;
}

/**
 * Caches resolved (title, artist) -> best YouTube video, so the same popular
 * track being played across guilds (or the same track re-queued) doesn't
 * repeat a YouTube search (or a metadata lookup). videoId/url/duration/
 * thumbnail are all safe to cache for the full TTL — none of them are a signed,
 * expiring stream URL (that's resolved separately, per-play, via
 * createYouTubeStreamGetter). Negative results (NoMatchFoundError) are
 * deliberately never cached: a transient search failure shouldn't be frozen
 * in place for the whole TTL.
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

  // Best-effort enrichment: without this, the panel falls back to a "live"
  // progress bar (null duration) and a "ソースを開く" link button instead of a
  // thumbnail (null thumbnailUrl) for every Spotify/Apple Music track, since
  // the search results above never carried duration/thumbnail data. A
  // metadata hiccup here must not fail the match itself - it only degrades
  // display back to the previous null/null behavior.
  let metadata: { durationMs: number | null; thumbnailUrl: string | null } = { durationMs: null, thumbnailUrl: null };
  try {
    metadata = await fetchYouTubeMetadata(best.videoId);
  } catch (err) {
    logger.warn({ err, videoId: best.videoId }, 'Failed to fetch matched video metadata - panel will show it as live/no-thumbnail');
  }

  const result: CachedMatch = { videoId: best.videoId, url: best.url, ...metadata };
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
  const { videoId, url, durationMs, thumbnailUrl } = await findBestYouTubeMatch(title, artist);
  return createQueueItem({
    title,
    artist,
    durationMs,
    thumbnailUrl,
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
 *
 * durationMs/thumbnailUrl aren't known yet at creation time (that's the whole
 * point of deferring the match), so the item starts with both null and the
 * panel would show it as "live"/no-thumbnail if it became current before
 * resolving - but resolveMatch() mutates this same QueueItem object in place
 * once the match completes, and the panel only ever displays a track once
 * it's actually `currentTrack` (never a queued-but-not-yet-playing one), by
 * which point prefetch (or worst case getStream() itself, inside startTrack,
 * before that track's own panel render) has already resolved it.
 */
export function createLazyMatchedQueueItem(params: MatchOnYouTubeParams): QueueItem {
  const { title, artist, requestedBy, sourceUrl, sourceType } = params;
  let memoized: CachedMatch | null = null;

  const item = createQueueItem({
    title,
    artist,
    durationMs: null,
    thumbnailUrl: null,
    sourceType,
    sourceUrl,
    requestedBy,
    getStream: async () => {
      const match = await resolveMatch();
      return createYouTubeStreamGetter(match.videoId, match.url)();
    },
    prefetch: async () => {
      await resolveMatch();
    },
  });

  async function resolveMatch(): Promise<CachedMatch> {
    memoized ??= await findBestYouTubeMatch(title, artist);
    item.durationMs = memoized.durationMs;
    item.thumbnailUrl = memoized.thumbnailUrl;
    return memoized;
  }

  return item;
}
