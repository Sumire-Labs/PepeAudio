import { createQueueItem, type QueueItem } from '../../player/QueueItem.js';
import { MAX_PLAYLIST_TRACKS } from '../../player/constants.js';
import { logger } from '../../logger.js';
import { getInnertube } from './innertubeClient.js';
import { createYouTubeStreamGetter } from './streamResolvers.js';
import { secondsToMs, VIDEO_ID_RE } from './parsing.js';
import { YouTubeUnavailableError } from './types.js';

/** Extracts a `list=` playlist id from a YouTube URL. */
export function extractPlaylistId(url: string): string | null {
  const m = /[?&]list=([\w-]+)/.exec(url);
  return m ? (m[1] ?? null) : null;
}

/**
 * Resolves a YouTube playlist URL to per-video QueueItems. Each entry already
 * carries a concrete video id, so items are built directly from the playlist
 * listing (title/author/duration) + a canonical watch URL — NO per-video
 * metadata fetch, so a 50-track playlist doesn't fan out into 50 lookups.
 *
 * SSRF-safe like the single-video path: every id is validated against
 * VIDEO_ID_RE and only a rebuilt canonical `watch?v=<id>` URL ever reaches the
 * downstream stream getter — the raw playlist URL is never forwarded.
 */
export async function resolveYouTubePlaylist(url: string, requestedBy: string): Promise<QueueItem[]> {
  const playlistId = extractPlaylistId(url);
  if (!playlistId) {
    throw new YouTubeUnavailableError('YouTubeプレイリストのIDを認識できませんでした。');
  }
  // Mix/radio playlists (RD…) are infinite/dynamic and auto-generated — reject
  // them rather than pull an unbounded, non-reproducible track set.
  if (/^(RD|UL)/.test(playlistId)) {
    throw new YouTubeUnavailableError('YouTubeのミックス/自動生成プレイリストは未対応です。通常のプレイリストをご利用ください。');
  }

  // youtubei.js's playlist tree shape varies by version — narrow defensively.
  interface RawVideo {
    id?: string;
    video_id?: string;
    title?: { text?: string };
    author?: { name?: string };
    duration?: { seconds?: number };
  }
  let videos: RawVideo[];
  try {
    const yt = await getInnertube();
    const playlist = (await yt.getPlaylist(playlistId)) as unknown as { videos?: RawVideo[] };
    videos = playlist.videos ?? [];
  } catch (err) {
    logger.error({ err, playlistId }, 'YouTube playlist resolution failed');
    throw new YouTubeUnavailableError('YouTubeプレイリストの取得に失敗しました。');
  }

  const items: QueueItem[] = [];
  for (const v of videos) {
    const videoId = v.video_id ?? v.id;
    if (!videoId || !VIDEO_ID_RE.test(videoId)) continue; // skip unavailable/private/non-video rows
    const canonicalUrl = `https://www.youtube.com/watch?v=${videoId}`;
    items.push(
      createQueueItem({
        title: v.title?.text ?? 'Unknown title',
        artist: v.author?.name ?? 'Unknown artist',
        durationMs: secondsToMs(v.duration?.seconds ?? null),
        thumbnailUrl: `https://i.ytimg.com/vi/${videoId}/mqdefault.jpg`,
        sourceType: 'youtube',
        sourceUrl: canonicalUrl,
        requestedBy,
        getStream: createYouTubeStreamGetter(videoId, canonicalUrl),
      }),
    );
    if (items.length >= MAX_PLAYLIST_TRACKS) break;
  }

  if (items.length === 0) {
    throw new YouTubeUnavailableError('プレイリスト内に再生できる動画が見つかりませんでした。');
  }
  return items;
}
