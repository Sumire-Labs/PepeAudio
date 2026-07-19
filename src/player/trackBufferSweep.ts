import { readdir, stat, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { logger } from '../logger.js';

/** Shared prefix for the per-track reseek buffer temp files (see PlaybackLifecycle.startTrackBuffer). */
export const TRACK_BUFFER_PREFIX = 'pepeaudio-buf-';

// A track buffer lives only minutes, so anything older is orphaned. The age gate
// also keeps the sweep safe under sharding: it can't delete a temp file another
// live shard is still writing.
const STALE_BUFFER_AGE_MS = 2 * 60 * 60 * 1000;

/**
 * Boot-time reclaim of orphaned reseek buffer temp files left by hard crashes
 * (in-process clearTrackBuffer covers normal exits). Never rejects - safe to
 * fire-and-forget at startup.
 */
export async function sweepStaleTrackBuffers(): Promise<void> {
  const dir = tmpdir();
  let entries: string[];
  try {
    entries = await readdir(dir);
  } catch (err) {
    logger.debug({ err }, 'Could not read tmpdir for stale track-buffer sweep');
    return;
  }

  const cutoff = Date.now() - STALE_BUFFER_AGE_MS;
  let removed = 0;
  await Promise.all(
    entries
      .filter((name) => name.startsWith(TRACK_BUFFER_PREFIX) && name.endsWith('.webm'))
      .map(async (name) => {
        const file = join(dir, name);
        try {
          const info = await stat(file);
          if (info.mtimeMs < cutoff) {
            await rm(file, { force: true });
            removed += 1;
          }
        } catch {
          // Raced with another process, or already gone - nothing to do.
        }
      }),
  );

  if (removed > 0) logger.info({ removed }, 'Swept orphaned track-buffer temp files at startup');
}
