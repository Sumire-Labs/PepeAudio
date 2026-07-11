import { rm } from 'node:fs/promises';
import { YtDlp, helpers as ytDlpHelpers } from 'ytdlp-nodejs';
import { logger } from '../../logger.js';

const ytDlpInstance = new YtDlp();
/**
 * Caches the readiness check itself as an in-flight promise (mirroring
 * innertubePromise in innertubeClient.ts) rather than a plain boolean — with a boolean, two
 * tracks resolving concurrently on a cold start could both see "not ready"
 * and both independently call checkInstallationAsync()/downloadYtDlp()
 * (a confirmed race: downloadYtDlp does a bare existsSync-then-write with no
 * lock/temp-file, so two concurrent downloads can corrupt each other).
 */
let ytDlpReadyPromise: Promise<void> | null = null;

/**
 * A freshly-downloaded yt-dlp binary is written to disk BEFORE its checksum is
 * verified, so a mismatch (or an unavailable checksum list) leaves an
 * unverified binary behind. Remove it — otherwise the next getYtDlp() call's
 * checkInstallationAsync() would silently adopt that unverified binary instead
 * of re-downloading and re-verifying.
 */
async function removeUnverifiedYtDlp(): Promise<void> {
  try {
    const binaryPath = ytDlpHelpers.findYtdlpBinary();
    if (binaryPath) {
      await rm(binaryPath, { force: true });
      logger.warn({ binaryPath }, 'Removed an unverified yt-dlp binary');
    }
  } catch (err) {
    logger.error({ err }, 'Failed to remove an unverified yt-dlp binary');
  }
}

/**
 * yt-dlp is the primary engine for actually streaming audio (see
 * createYouTubeStreamGetter for why) — this is the seam to check first if
 * playback starts failing across the board.
 */
export async function getYtDlp(): Promise<YtDlp> {
  if (!ytDlpReadyPromise) {
    ytDlpReadyPromise = ytDlpInstance.checkInstallationAsync().then(
      () => undefined,
      async () => {
        logger.warn('yt-dlp binary not found - attempting a checksum-verified download');
        // Use the checksum-verifying downloader (validates against the release's
        // SHA2-256SUMS) rather than the bare download. It throws on a checksum
        // MISMATCH and resolves with verified:false when the checksum list
        // itself couldn't be fetched — refuse to run the binary in either case
        // and clean it up, so we never execute an unverified yt-dlp. Callers
        // fall back to youtubei.js when this rejects.
        try {
          const result = await ytDlpHelpers.downloadYtDlpVerified();
          if (!result.verified) {
            throw new Error('yt-dlp download could not be checksum-verified (SHA2-256SUMS unavailable)');
          }
          logger.info({ checksum: result.checksum }, 'yt-dlp downloaded and checksum-verified');
        } catch (err) {
          await removeUnverifiedYtDlp();
          throw err;
        }
      },
    );
  }
  try {
    await ytDlpReadyPromise;
  } catch (err) {
    ytDlpReadyPromise = null; // let a subsequent call retry instead of permanently caching a failure
    throw err;
  }
  return ytDlpInstance;
}
