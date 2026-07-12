import spotifyUrlInfoFactory from 'spotify-url-info';
import type { QueueItem } from '../player/QueueItem.js';
import { matchOnYouTube, createLazyMatchedQueueItem } from './youtubeMatch.js';
import { MAX_PLAYLIST_TRACKS } from '../player/constants.js';
import { isSpotifyHost } from '../util/urlPatterns.js';
import { logger } from '../logger.js';
import { safeExtractTitleArtist } from './spotifyTrackParser.js';

export { NoMatchFoundError } from './youtubeMatch.js';
export class SpotifyResolutionError extends Error {}

const spotify = spotifyUrlInfoFactory(fetch);

/**
 * Defense-in-depth before handing a URL to spotify-url-info (which fetches
 * whatever URL it derives from the input). classifyInput() already routes only
 * Spotify hosts here, but — mirroring sanitizeSoundCloudUrl — this resolver
 * re-validates the parsed host rather than trusting the caller, then rebuilds a
 * clean https URL dropping any embedded credentials, port, and fragment. Ensures
 * no resolver relies solely on its caller for the SSRF host check.
 */
function sanitizeSpotifyUrl(rawUrl: string): string {
  let parsed: URL;
  try {
    parsed = new URL(rawUrl);
  } catch {
    throw new SpotifyResolutionError('SpotifyのURLを認識できませんでした。');
  }
  if (!isSpotifyHost(parsed.hostname.toLowerCase())) {
    throw new SpotifyResolutionError('SpotifyのURLではありません。');
  }
  return `https://${parsed.hostname}${parsed.pathname}${parsed.search}`;
}

export async function resolveSpotifyUrl(rawUrl: string, requestedBy: string): Promise<QueueItem[]> {
  const url = sanitizeSpotifyUrl(rawUrl);
  const isPlaylistOrAlbum = /\/(playlist|album)\//.test(url);

  if (isPlaylistOrAlbum) {
    let rawData: { trackList?: unknown[] };
    try {
      rawData = (await spotify.getData(url)) as { trackList?: unknown[] };
    } catch (err) {
      logger.error({ err, url }, 'Spotify playlist/album metadata resolution failed');
      throw new SpotifyResolutionError('Spotifyのプレイリスト/アルバム情報の取得に失敗しました。');
    }

    const allRawTracks = rawData.trackList ?? [rawData];
    // Cap resolution work per request — a huge playlist would otherwise trigger
    // one YouTube search per track and exhaust the host (see MAX_PLAYLIST_TRACKS).
    const rawTracks = allRawTracks.slice(0, MAX_PLAYLIST_TRACKS);
    if (allRawTracks.length > rawTracks.length) {
      logger.info(
        { url, total: allRawTracks.length, cap: MAX_PLAYLIST_TRACKS },
        'Spotify playlist/album exceeds the per-request track cap - resolving only the first tracks',
      );
    }
    // YouTube matching is deferred to each item's first getStream() call (see
    // createLazyMatchedQueueItem) rather than done eagerly here - a large
    // playlist would otherwise block this command's response on one YouTube
    // search per track. Only metadata extraction can fail synchronously here;
    // an actual "no YouTube match" failure now surfaces later, per-track, via
    // GuildPlayer's existing playback-failure skip-to-next handling.
    const items: QueueItem[] = [];
    let failed = 0;
    for (const rawTrack of rawTracks) {
      const parsed = safeExtractTitleArtist(rawTrack);
      if (!parsed) {
        failed += 1;
        logger.warn({ url }, 'Skipping one Spotify playlist/album entry with an unrecognizable shape');
        continue;
      }
      items.push(
        createLazyMatchedQueueItem({ title: parsed.title, artist: parsed.artist, requestedBy, sourceUrl: url, sourceType: 'spotify' }),
      );
    }
    if (items.length === 0) {
      throw new SpotifyResolutionError('プレイリスト内の曲情報を1つも読み取れませんでした。');
    }
    if (failed > 0) {
      logger.info({ parsed: items.length, failed }, 'Spotify playlist/album partially parsed (YouTube matching happens lazily per track)');
    }
    return items;
  }

  let rawData: unknown;
  try {
    rawData = await spotify.getData(url);
  } catch (err) {
    logger.error({ err, url }, 'Spotify track metadata resolution failed');
    throw new SpotifyResolutionError('Spotifyのトラック情報の取得に失敗しました。');
  }
  const parsed = safeExtractTitleArtist(rawData);
  if (!parsed) {
    throw new SpotifyResolutionError('Spotifyのトラック情報を読み取れませんでした。');
  }
  return [
    await matchOnYouTube({ title: parsed.title, artist: parsed.artist, requestedBy, sourceUrl: url, sourceType: 'spotify' }),
  ];
}
