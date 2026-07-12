import { classifyInput, normalizeUrlInput } from '../util/urlPatterns.js';
import { resolveYouTubeUrl, resolveYouTubeVideoId, searchYouTube } from './youtube.js';
import { resolveSpotifyUrl } from './spotify.js';
import { resolveSoundCloudUrl } from './soundcloud.js';
import { resolveAppleMusicUrl } from './appleMusic.js';
import { escapeMd } from '../ui/panelMarkdown.js';
import type { QueueItem } from '../player/QueueItem.js';

export class SourceResolutionError extends Error {}

export { YouTubeUnavailableError } from './youtube.js';
export { NoMatchFoundError, SpotifyResolutionError } from './spotify.js';
export { SoundCloudUnavailableError } from './soundcloud.js';
export { AppleMusicResolutionError } from './appleMusic.js';

/** Single entry point `/play` uses: classifies the input and dispatches to the right resolver. */
export async function resolveInput(query: string, requestedBy: string): Promise<QueueItem[]> {
  const kind = classifyInput(query);
  // Downstream URL parsing (new URL(), spotify-uri) requires a scheme; a link
  // pasted as e.g. "youtube.com/watch?v=..." classifies fine but previously
  // threw deep inside extractVideoId()/spotify-uri with a confusing error.
  const normalized = kind === 'search' ? query : normalizeUrlInput(query);

  switch (kind) {
    case 'youtube':
      return resolveYouTubeUrl(normalized, requestedBy);
    case 'spotify':
      return resolveSpotifyUrl(normalized, requestedBy);
    case 'soundcloud':
      return resolveSoundCloudUrl(normalized, requestedBy);
    case 'applemusic':
      return resolveAppleMusicUrl(normalized, requestedBy);
    case 'search': {
      const results = await searchYouTube(query, 1);
      const top = results[0];
      if (!top) {
        throw new SourceResolutionError(`「${escapeMd(query)}」に一致する動画が見つかりませんでした。`);
      }
      return [await resolveYouTubeVideoId(top.videoId, top.url, requestedBy)];
    }
  }
}
