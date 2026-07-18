import { parseYouTubeTimestamp } from '../../util/timestamp.js';
import type { QueueItem } from '../../player/QueueItem.js';
import { extractVideoId, VIDEO_ID_RE } from './parsing.js';
import { resolveYouTubeVideoId } from './queueItemBuilders.js';
import { resolveYouTubePlaylist } from './playlist.js';
import { YouTubeUnavailableError } from './types.js';

export async function resolveYouTubeUrl(url: string, requestedBy: string): Promise<QueueItem[]> {
  // A bare playlist link (list= with no watch?v=) → import the whole playlist.
  // A video-in-a-playlist link (watch?v=…&list=…) still plays just that video
  // (the list= is dropped below), matching the prior single-video behavior.
  if (/[?&]list=/.test(url) && !/watch\?v=/.test(url)) {
    return resolveYouTubePlaylist(url, requestedBy);
  }
  const videoId = extractVideoId(url);
  if (!videoId || !VIDEO_ID_RE.test(videoId)) {
    throw new YouTubeUnavailableError('そのリンクからYouTubeの動画IDを認識できませんでした。');
  }
  // Never forward the raw user URL to yt-dlp/youtubei — rebuild a canonical
  // watch URL from the validated 11-char ID. This is the last line of defense
  // against SSRF: whatever the user pasted, the downloader only ever sees
  // `https://www.youtube.com/watch?v=<safe id>`. It also drops stray params
  // (e.g. a `list=` that would otherwise pull an entire playlist).
  const canonicalUrl = `https://www.youtube.com/watch?v=${videoId}`;
  const item = await resolveYouTubeVideoId(videoId, canonicalUrl, requestedBy);
  // Parsed from the original pasted URL, not the rebuilt canonical one (which
  // deliberately drops all query params other than v= — see above).
  item.initialOffsetMs = parseYouTubeTimestamp(url);
  return [item];
}
