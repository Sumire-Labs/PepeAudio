import { rm } from 'node:fs/promises';
import { YtDlp, helpers as ytDlpHelpers } from 'ytdlp-nodejs';
import { logger } from '../../logger.js';

const ytDlpInstance = new YtDlp();
// In-flight promise (not a boolean) so concurrent cold-start downloads serialize — the
// underlying download has no lock and would otherwise race and corrupt each other.
let ytDlpReadyPromise: Promise<void> | null = null;

// The binary is written to disk before its checksum is verified; remove it on failure so the
// next getYtDlp() doesn't silently adopt an unverified binary instead of re-downloading.
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

export async function getYtDlp(): Promise<YtDlp> {
  if (!ytDlpReadyPromise) {
    ytDlpReadyPromise = ytDlpInstance.checkInstallationAsync().then(
      () => undefined,
      async () => {
        logger.warn('yt-dlp binary not found - attempting a checksum-verified download');
        // downloadYtDlpVerified throws on a checksum MISMATCH but resolves verified:false when the
        // SHA2-256SUMS list can't be fetched — refuse and clean up in both cases so we never run an
        // unverified binary. Callers fall back to youtubei.js when this rejects.
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
