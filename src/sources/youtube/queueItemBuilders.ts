import { createQueueItem, type QueueItem } from '../../player/QueueItem.js';
import { logger } from '../../logger.js';
import { getInnertube } from './innertubeClient.js';
import { getYtDlp } from './ytdlpClient.js';
import { secondsToMs } from './parsing.js';
import { createYouTubeStreamGetter, resolveStreamViaYtDlp } from './streamResolvers.js';
import type { YtDlpVideoInfo } from './types.js';

async function buildFromInnertube(videoId: string, sourceUrl: string, requestedBy: string): Promise<QueueItem> {
  const yt = await getInnertube();
  const info = await yt.getBasicInfo(videoId);
  const basicInfo = info.basic_info;

  return createQueueItem({
    title: basicInfo.title ?? 'Unknown title',
    artist: basicInfo.author ?? 'Unknown artist',
    durationMs: secondsToMs(basicInfo.duration ?? null),
    thumbnailUrl: basicInfo.thumbnail?.[0]?.url ?? null,
    sourceType: 'youtube',
    sourceUrl,
    requestedBy,
    getStream: createYouTubeStreamGetter(videoId, sourceUrl),
  });
}

async function buildFromYtDlp(sourceUrl: string, requestedBy: string): Promise<QueueItem> {
  const ytdlp = await getYtDlp();
  const info = (await ytdlp.getInfoAsync(sourceUrl)) as YtDlpVideoInfo;
  return createQueueItem({
    title: info.title ?? 'Unknown title',
    artist: info.uploader ?? info.channel ?? 'Unknown artist',
    durationMs: secondsToMs(info.duration ?? null),
    thumbnailUrl: info.thumbnail ?? null,
    sourceType: 'youtube',
    sourceUrl,
    requestedBy,
    getStream: () => resolveStreamViaYtDlp(sourceUrl),
  });
}

export async function resolveYouTubeVideoId(videoId: string, sourceUrl: string, requestedBy: string): Promise<QueueItem> {
  try {
    return await buildFromInnertube(videoId, sourceUrl, requestedBy);
  } catch (err) {
    logger.warn({ err, videoId }, 'youtubei.js metadata resolution failed - falling back to yt-dlp metadata');
    return buildFromYtDlp(sourceUrl, requestedBy);
  }
}

export interface YouTubeMetadata {
  durationMs: number | null;
  thumbnailUrl: string | null;
}

/**
 * Lightweight metadata-only lookup for a video whose id is already known (used
 * by youtubeMatch.ts to enrich a Spotify/Apple Music match with the matched
 * video's real duration/thumbnail, instead of the panel falling back to a
 * "live" progress bar + no-thumbnail state). Deliberately no yt-dlp fallback
 * unlike resolveYouTubeVideoId above - this is a display nice-to-have, not
 * required for playback, so a failure here degrades to null/null rather than
 * blocking or retrying.
 */
export async function fetchYouTubeMetadata(videoId: string): Promise<YouTubeMetadata> {
  const yt = await getInnertube();
  const info = await yt.getBasicInfo(videoId);
  return {
    durationMs: secondsToMs(info.basic_info.duration ?? null),
    thumbnailUrl: info.basic_info.thumbnail?.[0]?.url ?? null,
  };
}
