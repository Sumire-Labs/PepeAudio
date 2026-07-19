import { Readable } from 'node:stream';
import { StreamType } from '@discordjs/voice';
import { getYtDlp } from './ytdlpClient.js';
import { getInnertube } from './innertubeClient.js';
import { logger } from '../../logger.js';

/**
 * Hardcode StreamType.WebmOpus (yt-dlp audio-only is always itag 251 Opus/WebM)
 * to skip @discordjs/voice's expensive Arbitrary probe/transcode. Don't use
 * demuxProbe(): the stream it returns silently stalls.
 */
export async function resolveStreamViaYtDlp(url: string): Promise<{ stream: Readable; inputType?: StreamType }> {
  const ytdlp = await getYtDlp();
  const stream = ytdlp.stream(url).filter('audioonly').getStream() as unknown as Readable;
  return { stream, inputType: StreamType.WebmOpus };
}

/**
 * format:'webm'/codec:'opus' is a real filter, not a hint: youtubei.js's
 * chooseFormat() throws if nothing matches both rather than substituting, so a
 * resolved stream is always WebM/Opus and StreamType.WebmOpus is safe to
 * hardcode without probing.
 */
async function resolveStreamViaInnertube(videoId: string): Promise<{ stream: Readable; inputType?: StreamType }> {
  const client = await getInnertube();
  // youtubei.js returns a web ReadableStream; @discordjs/voice needs a Node Readable.
  const webStream = (await client.download(videoId, {
    type: 'audio',
    quality: 'best',
    format: 'webm',
    codec: 'opus',
  })) as unknown as Parameters<typeof Readable.fromWeb>[0];
  const stream = Readable.fromWeb(webStream);
  return { stream, inputType: StreamType.WebmOpus };
}

/**
 * yt-dlp is tried FIRST: this youtubei.js version's download() deciphering fails
 * consistently ("No valid URL to decipher") though its metadata/search calls
 * work. youtubei.js stays as fallback if a future release fixes deciphering or
 * yt-dlp is unavailable.
 */
export function createYouTubeStreamGetter(videoId: string, fallbackUrl: string) {
  return async (): Promise<{ stream: Readable; inputType?: StreamType }> => {
    try {
      return await resolveStreamViaYtDlp(fallbackUrl);
    } catch (err) {
      logger.warn({ err, videoId }, 'yt-dlp stream extraction failed - falling back to youtubei.js');
      return resolveStreamViaInnertube(videoId);
    }
  };
}
