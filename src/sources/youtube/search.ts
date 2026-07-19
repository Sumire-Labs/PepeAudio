import { YTNodes } from 'youtubei.js';
import { logger } from '../../logger.js';
import { getInnertube } from './innertubeClient.js';
import { getYtDlp } from './ytdlpClient.js';
import type { YouTubeSearchResult, YtDlpVideoInfo } from './types.js';

export async function searchYouTube(query: string, limit = 1): Promise<YouTubeSearchResult[]> {
  try {
    const yt = await getInnertube();
    const search = await yt.search(query, { type: 'video' });
    // youtubei.js result shape is version-fragile; narrow defensively.
    const videos = ((search as unknown as { results?: { filterType: (t: unknown) => unknown[] } }).results?.filterType(
      YTNodes.Video,
    ) ?? []) as Array<{ video_id: string; title?: { text?: string }; author?: { name?: string }; length_text?: { text?: string } }>;

    if (videos.length > 0) {
      return videos.slice(0, limit).map((v) => ({
        videoId: v.video_id,
        title: v.title?.text ?? query,
        author: v.author?.name ?? 'Unknown',
        url: `https://www.youtube.com/watch?v=${v.video_id}`,
      }));
    }
  } catch (err) {
    logger.warn({ err, query }, 'youtubei.js search failed - falling back to yt-dlp search');
  }

  try {
    const ytdlp = await getYtDlp();
    const info = (await ytdlp.getInfoAsync(`ytsearch1:${query}`)) as YtDlpVideoInfo;
    if (info?.id) {
      return [
        {
          videoId: info.id,
          title: info.title ?? query,
          author: info.uploader ?? info.channel ?? 'Unknown',
          url: info.webpage_url ?? `https://www.youtube.com/watch?v=${info.id}`,
        },
      ];
    }
  } catch (err) {
    logger.error({ err, query }, 'yt-dlp search fallback also failed');
  }

  return [];
}
