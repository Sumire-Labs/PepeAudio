import { createQueueItem, type QueueItem } from '../../player/QueueItem.js';
import { logger } from '../../logger.js';
import { AUTOPLAY_FETCH_LIMIT } from '../../player/constants.js';
import { getInnertube } from './innertubeClient.js';
import { extractVideoId, VIDEO_ID_RE } from './parsing.js';
import { searchYouTube } from './search.js';
import { createYouTubeStreamGetter } from './streamResolvers.js';

interface RelatedTrack {
  videoId: string;
  title: string;
  author: string;
  durationMs: number | null;
  thumbnailUrl: string | null;
}

/**
 * Pulls the "up next / related" sidebar feed (`watch_next_feed`) for a YouTube
 * video via youtubei.js — the same signal YouTube's own autoplay is built on.
 *
 * In youtubei.js 17.x the feed items are `LockupView` nodes (verified against a
 * live response): the video id lives in `content_id`, the title in
 * `metadata.title.text`, and the channel in the first metadata row. `content_type`
 * separates real videos from the playlist/mix lockups and the Shorts `ReelShelf`,
 * so only `'VIDEO'` lockups are kept. Every field is read defensively because
 * these node shapes drift between youtubei.js versions. Thumbnails are derived
 * from the video id (deterministic) rather than parsed out of the lockup.
 */
async function fetchRelatedYouTube(videoId: string, limit: number): Promise<RelatedTrack[]> {
  const yt = await getInnertube();
  const info = await yt.getInfo(videoId);
  const feed = ((info as unknown as { watch_next_feed?: unknown[] }).watch_next_feed ?? []) as Array<{
    type?: string;
    content_type?: string;
    content_id?: string;
    metadata?: {
      title?: { text?: string };
      metadata?: { metadata_rows?: Array<{ metadata_parts?: Array<{ text?: { text?: string } }> }> };
    };
  }>;

  const out: RelatedTrack[] = [];
  for (const node of feed) {
    if (node.type !== 'LockupView' || node.content_type !== 'VIDEO') continue;
    const vid = node.content_id;
    if (!vid || !VIDEO_ID_RE.test(vid)) continue;
    const author = node.metadata?.metadata?.metadata_rows?.[0]?.metadata_parts?.[0]?.text?.text;
    out.push({
      videoId: vid,
      title: node.metadata?.title?.text ?? 'Unknown title',
      author: author ?? 'Unknown artist',
      // Duration isn't reliably exposed on the lockup; the panel degrades to an
      // elapsed-only display until the track actually plays.
      durationMs: null,
      thumbnailUrl: `https://i.ytimg.com/vi/${vid}/hqdefault.jpg`,
    });
    if (out.length >= limit) break;
  }
  return out;
}

/**
 * Autoplay / "radio" source: given the track that just finished, returns a batch
 * of related YouTube tracks to keep the session going. The related feed is
 * seeded from the finished track's YouTube video id — extracted directly for
 * native YouTube items, or found via a `<artist> <title>` search for
 * Spotify/Apple/SoundCloud items (which are all matched to YouTube for playback
 * anyway). Any failure degrades to an empty list so the caller just stops rather
 * than surfacing an error. De-duplication against session history is the
 * caller's job (see QueueHistoryManager).
 */
export async function resolveAutoplayTracks(seed: QueueItem): Promise<QueueItem[]> {
  try {
    let seedVideoId = seed.sourceType === 'youtube' ? extractVideoId(seed.sourceUrl) : null;
    if (!seedVideoId) {
      const query = `${seed.artist} ${seed.title}`.trim();
      const results = await searchYouTube(query, 1);
      seedVideoId = results[0]?.videoId ?? null;
    }
    if (!seedVideoId) return [];

    const related = await fetchRelatedYouTube(seedVideoId, AUTOPLAY_FETCH_LIMIT);
    return related.map((r) => {
      const url = `https://www.youtube.com/watch?v=${r.videoId}`;
      return createQueueItem({
        title: r.title,
        artist: r.author,
        durationMs: r.durationMs,
        thumbnailUrl: r.thumbnailUrl,
        sourceType: 'youtube',
        sourceUrl: url,
        requestedBy: seed.requestedBy,
        getStream: createYouTubeStreamGetter(r.videoId, url),
      });
    });
  } catch (err) {
    logger.warn({ err, seed: seed.title }, 'Autoplay: failed to resolve related tracks');
    return [];
  }
}
