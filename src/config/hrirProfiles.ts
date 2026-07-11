import { readdirSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { logger } from '../logger.js';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const PROJECT_ROOT = path.resolve(__dirname, '..', '..');

/** Gitignored — see assets/hrir_profiles/README.md. Nothing here is bundled or downloaded by this bot. */
export const DEFAULT_HRIR_PROFILES_DIR = path.join(PROJECT_ROOT, 'assets', 'hrir_profiles');

/** Bounds how long a single ffmpeg channel-count probe may run before we give up on that file. */
const PROBE_TIMEOUT_MS = 10_000;

export type HrirFormat = 'simple' | 'hesuvi14';

export interface HrirProfile {
  /** Stable id, persisted as the guild's applied profile — the filename without extension. */
  id: string;
  /** Absolute path to the WAV impulse-response file. */
  filePath: string;
  /**
   * 'simple': plain mono/stereo IR, used directly by ffmpegFilters.HRIR_SIMPLE_FILTER_COMPLEX.
   * 'hesuvi14': a genuine HeSuVi-style 14-channel HRIR (see ffmpegFilters.HRIR_HESUVI14_FILTER_COMPLEX
   * for the exact channel mapping this was verified against).
   */
  format: HrirFormat;
}

/**
 * Reads the exact channel count via ffmpeg's `ashowinfo` filter (prints a numeric
 * `channels:N` field to stderr) rather than parsing the human-readable "Guessed
 * Channel Layout" name ffmpeg logs by default - that's just a display label for
 * well-known channel counts (e.g. any 14-channel file gets called "9.1.4" even
 * though HeSuVi's own files aren't really a 9.1.4 immersive-audio signal) and
 * isn't a reliable way to get the actual number. ffprobe would give this more
 * directly but isn't bundled alongside our resolved ffmpeg binary.
 * Bounded by PROBE_TIMEOUT_MS — this runs synchronously at startup, before the
 * Discord client logs in and with no watchdog; a stalled ffmpeg process on a
 * corrupt/pathological file would otherwise hang the whole bot indefinitely.
 */
function probeChannelCount(ffmpegPath: string, filePath: string): number | null {
  const result = spawnSync(
    ffmpegPath,
    ['-y', '-i', filePath, '-af', 'ashowinfo', '-frames:a', '1', '-f', 'null', '-'],
    { encoding: 'utf8', timeout: PROBE_TIMEOUT_MS },
  );
  if (result.error || result.signal) {
    logger.warn(
      { filePath, error: result.error, signal: result.signal },
      'ffmpeg channel-count probe failed or timed out - skipping this HRIR file',
    );
    return null;
  }
  const output = `${result.stdout ?? ''}${result.stderr ?? ''}`;
  const match = output.match(/channels:(\d+)/);
  return match ? parseInt(match[1]!, 10) : null;
}

/**
 * Classifies a probed channel count into a supported format, or null if
 * unsupported. 14 channels is HeSuVi's standard "with reverb" HRIR shape
 * (verified 2026-07-11 against real atmos.wav/dht.wav files from an actual
 * HeSuVi install — see ffmpegFilters.ts for the channel-mapping details).
 * HeSuVi's "no reverb" (`-` suffixed) files use a different, still-unverified
 * 7-channel layout and are deliberately not supported - shipping a guessed
 * mapping there risks a silently-wrong (not crashing) result.
 */
function classifyFormat(channelCount: number | null): HrirFormat | null {
  if (channelCount === 1 || channelCount === 2) return 'simple';
  if (channelCount === 14) return 'hesuvi14';
  return null;
}

/**
 * Scans a local directory for a user-supplied HRIR/BRIR WAV file (bring-your-own —
 * see assets/hrir_profiles/README.md for why none are bundled). Missing directory
 * is not an error: the feature is simply unavailable until someone populates it.
 * Only ONE profile is ever used (GuildPlayer applies whichever this returns as
 * its single element — there is no per-guild selection), so this stops probing
 * as soon as it finds the first (alphabetically) file with a supported channel
 * count, rather than spawning ffmpeg against every file in the folder.
 */
export function loadHrirProfiles(ffmpegPath: string, dir: string = DEFAULT_HRIR_PROFILES_DIR): HrirProfile[] {
  let entries: string[];
  try {
    entries = readdirSync(dir);
  } catch {
    return [];
  }
  const wavFiles = entries.filter((f) => f.toLowerCase().endsWith('.wav')).sort();

  for (const filename of wavFiles) {
    const filePath = path.join(dir, filename);
    const channelCount = probeChannelCount(ffmpegPath, filePath);
    const format = classifyFormat(channelCount);
    if (!format) {
      logger.warn(
        { filename, channelCount },
        'Skipping HRIR file with an unsupported channel count (expected 1, 2, or 14)',
      );
      continue;
    }
    const id = filename.slice(0, -'.wav'.length);
    return [{ id, filePath, format }];
  }
  return [];
}
