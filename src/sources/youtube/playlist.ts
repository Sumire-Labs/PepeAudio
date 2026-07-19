import { createQueueItem, type QueueItem } from '../../player/QueueItem.js';
import { MAX_PLAYLIST_TRACKS } from '../../player/constants.js';
import { logger } from '../../logger.js';
import { getInnertube } from './innertubeClient.js';
import { createYouTubeStreamGetter } from './streamResolvers.js';
import { secondsToMs, VIDEO_ID_RE } from './parsing.js';
import { YouTubeUnavailableError } from './types.js';

export function extractPlaylistId(url: string): string | null {
  const m = /[?&]list=([\w-]+)/.exec(url);
  return m ? (m[1] ?? null) : null;
}

// SSRF guard: only VIDEO_ID_RE-validated ids reach the stream getter as rebuilt canonical watch URLs; the raw playlist URL is never forwarded.
export async function resolveYouTubePlaylist(url: string, requestedBy: string): Promise<QueueItem[]> {
  const playlistId = extractPlaylistId(url);
  if (!playlistId) {
    throw new YouTubeUnavailableError('YouTubeプレイリストのIDを認識できませんでした。');
  }
  // Reject Mix/radio playlists (RD/UL): infinite, auto-generated, non-reproducible.
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
    if (!videoId || !VIDEO_ID_RE.test(videoId)) continue;
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
