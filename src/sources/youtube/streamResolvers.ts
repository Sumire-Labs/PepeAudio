import { Readable } from 'node:stream';
import { StreamType } from '@discordjs/voice';
import { getYtDlp } from './ytdlpClient.js';
import { getInnertube } from './innertubeClient.js';
import { logger } from '../../logger.js';

/**
 * StreamType.Arbitrary forces @discordjs/voice to spin up its own internal
 * ffmpeg probe+transcode on a live, non-seekable, no-duration WebM pipe
 * (yt-dlp's audio-only output) - the discord.js guide itself calls this "one
 * of the most computationally demanding parts of the audio pipeline," and it's
 * the most likely source of the "audio briefly speeds up" glitch reported
 * during normal playback.
 *
 * Tried @discordjs/voice's demuxProbe() here first (it's built for exactly
 * this: detect WebM/Ogg Opus via a lightweight header parse instead of a full
 * ffmpeg probe) — confirmed by direct testing that it correctly identifies
 * yt-dlp's stream as 'webm/opus', but the stream it hands back never actually
 * emits data afterward (silently stalls, no error). It's marked @experimental
 * in @discordjs/voice's own types; not usable here as a result.
 *
 * yt-dlp's audio-only selection for YouTube is empirically always itag 251
 * (Opus-in-WebM) as of this writing (confirmed via direct testing across
 * several videos), so StreamType.WebmOpus is hardcoded rather than probed -
 * this still skips the "Arbitrary" full-probe/transcode path. If yt-dlp ever
 * selects a genuinely different container for some video, @discordjs/voice's
 * WebM/Opus demuxer will fail fast on it, and the existing playback-failure
 * retry/skip handling in GuildPlayer takes over rather than silent failure.
 */
export async function resolveStreamViaYtDlp(url: string): Promise<{ stream: Readable; inputType?: StreamType }> {
  const ytdlp = await getYtDlp();
  const stream = ytdlp.stream(url).filter('audioonly').getStream() as unknown as Readable;
  return { stream, inputType: StreamType.WebmOpus };
}

/**
 * Requesting format:'webm'/codec:'opus' is a genuine filter, not a hint —
 * verified directly against youtubei.js's chooseFormat() source
 * (utils/FormatUtils.js): both options narrow the candidate list by checking
 * `format.mime_type.includes(...)`, and if nothing matches both, it throws
 * ("No matching formats found") rather than silently substituting a different
 * container/codec. So a resolved stream here is *always* genuinely WebM/Opus
 * (itag 251, same as yt-dlp's audio-only selection) or this rejects outright -
 * StreamType.WebmOpus is safe to hardcode without probing, skipping
 * @discordjs/voice's expensive Arbitrary full-probe/transcode path. A throw
 * here propagates like any other stream-resolution failure, into the same
 * existing playback-failure retry/skip handling in GuildPlayer.
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
 * Reusable lazy stream getter — shared by direct YouTube links and the Spotify→YouTube match path.
 * yt-dlp is tried FIRST here, not youtubei.js — confirmed by direct testing (across several
 * videos, all `client` types youtubei.js exposes) that this youtubei.js version's signature
 * deciphering for `download()` fails consistently ("No valid URL to decipher"), while metadata/
 * search calls (getBasicInfo/search) work fine. yt-dlp is kept as primary for the actual stream;
 * youtubei.js remains a fallback here in case a future release fixes deciphering, or yt-dlp itself
 * is unavailable on a given host.
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
