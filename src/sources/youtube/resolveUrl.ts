import { parseYouTubeTimestamp } from '../../util/timestamp.js';
import type { QueueItem } from '../../player/QueueItem.js';
import { extractVideoId, VIDEO_ID_RE } from './parsing.js';
import { resolveYouTubeVideoId } from './queueItemBuilders.js';
import { resolveYouTubePlaylist } from './playlist.js';
import { YouTubeUnavailableError } from './types.js';

export async function resolveYouTubeUrl(url: string, requestedBy: string): Promise<QueueItem[]> {
  // Bare list= (no watch?v=) imports the whole playlist; watch?v=…&list=… plays just the video (list dropped below).
  if (/[?&]list=/.test(url) && !/watch\?v=/.test(url)) {
    return resolveYouTubePlaylist(url, requestedBy);
  }
  const videoId = extractVideoId(url);
  if (!videoId || !VIDEO_ID_RE.test(videoId)) {
    throw new YouTubeUnavailableError('そのリンクからYouTubeの動画IDを認識できませんでした。');
  }
  // SSRF guard: rebuild a canonical watch URL from the validated 11-char ID so the downloader never sees the raw user URL (also drops stray params like list=).
  const canonicalUrl = `https://www.youtube.com/watch?v=${videoId}`;
  const item = await resolveYouTubeVideoId(videoId, canonicalUrl, requestedBy);
  // Timestamp comes from the original URL — the canonical one drops all params except v=.
  item.initialOffsetMs = parseYouTubeTimestamp(url);
  return [item];
}
