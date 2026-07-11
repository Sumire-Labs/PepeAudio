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
