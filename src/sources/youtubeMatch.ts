import { createQueueItem, type QueueItem, type SourceType } from '../player/QueueItem.js';
import { searchYouTube, createYouTubeStreamGetter, fetchYouTubeMetadata } from './youtube.js';
import { LruCache } from '../util/lruCache.js';
import { escapeMd } from '../ui/panelMarkdown.js';
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

// Never cache the signed, expiring stream URL (resolved per-play). Negative
// results are deliberately not cached so a transient failure isn't frozen for the TTL.
const MATCH_CACHE_MAX_SIZE = 1_000;
const MATCH_CACHE_TTL_MS = 6 * 60 * 60 * 1_000;
const matchCache = new LruCache<string, CachedMatch>(MATCH_CACHE_MAX_SIZE, MATCH_CACHE_TTL_MS);

async function findBestYouTubeMatch(title: string, artist: string): Promise<CachedMatch> {
  const cacheKey = `${title} ${artist}`.trim().toLowerCase();
  const cached = matchCache.get(cacheKey);
  if (cached) return cached;

  const candidates = await searchYouTube(cacheKey, 5);
  if (candidates.length === 0) {
    throw new NoMatchFoundError(`「${escapeMd(title)}」に一致するYouTube動画が見つかりませんでした。`);
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

  // A metadata hiccup must not fail the match — it only degrades the panel to live/no-thumbnail.
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
 * Matches EAGERLY — for single-track links, where the /play response itself
 * should say "not found" if nothing matches. Shared by Spotify and Apple Music.
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
 * LAZY variant: the match is deferred to the first getStream() call — for
 * playlist/album entries, so /play isn't blocked on one search per track — and
 * memoized so loop/previous/retry replays don't re-search. A match failure
 * surfaces as a getStream() rejection, handled by GuildPlayer's existing
 * skip-to-next path. durationMs/thumbnailUrl start null; resolveMatch() mutates
 * them on this same item in place before the panel ever renders it as current.
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
