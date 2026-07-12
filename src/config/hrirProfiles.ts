import { readdirSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { logger } from '../logger.js';
import { buildHrirMeasureGraph } from '../audio/hrirFilterComplex.js';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const PROJECT_ROOT = path.resolve(__dirname, '..', '..');

/** Gitignored — see assets/hrir_profiles/README.md. Nothing here is bundled or downloaded by this bot. */
export const DEFAULT_HRIR_PROFILES_DIR = path.join(PROJECT_ROOT, 'assets', 'hrir_profiles');

/** Bounds how long a single ffmpeg channel-count probe may run before we give up on that file. */
const PROBE_TIMEOUT_MS = 10_000;

/** Bounds the (one-per-loaded-profile) makeup-gain measurement convolution at startup. */
const MAKEUP_MEASURE_TIMEOUT_MS = 20_000;

/**
 * Headroom (dB) left BELOW a perfect RMS level-match, so hot / loudness-war
 * masters don't ride continuously into the safety limiter. Measured: without it,
 * a ~-9 dBFS-RMS master convolves + makes up to peaks around +3.7 dBFS and
 * alimiter=0.95 clamps ~3-4 dB nonstop — audible over-limiting ("harsh"/pumping).
 * With this margin, hot masters land near the limiter threshold instead of
 * slamming into it, so the limiter is a true safety net. Trades a few dB of
 * loudness (recover it with the volume control) for a clean, un-pumped signal.
 * Tunable by ear.
 */
const MAKEUP_HEADROOM_DB = 4;

/**
 * Fallback makeup used only if the measurement below fails to produce a number
 * (ffmpeg error/timeout/unparseable output). Room BRIRs cluster around ~20 dB
 * of insertion loss; this is a deliberately conservative under-estimate (already
 * minus the headroom above) so the fallback never over-boosts into the limiter.
 * Real deployments measure their own value and never hit this.
 */
const DEFAULT_HRIR_MAKEUP_DB = 14;

/**
 * Decorrelated pink-noise reference signal, generated entirely in-process by
 * ffmpeg's lavfi source (no temp files). Decorrelated (two seeds joined to
 * stereo) rather than mono-duplicated because a stereo master's SIDE energy is
 * what the BRIR spatialises — a correlated signal under-reads the level loss by
 * ~2 dB. `join` with an explicit stereo layout is required so the downstream
 * `pan=...|c0=FL...` can address channels by name; lavfi rejects a trailing
 * output pad label, so the graph deliberately ends unlabelled.
 */
const MAKEUP_NOISE_LAVFI =
  'anoisesrc=color=pink:amplitude=0.2:duration=4:seed=1:sample_rate=48000[l];' +
  'anoisesrc=color=pink:amplitude=0.2:duration=4:seed=2:sample_rate=48000[r];' +
  '[l][r]join=inputs=2:channel_layout=stereo';

/** Parses the LAST `RMS level dB:` astats reports (its Overall block, printed after per-channel), or null. */
function parseLastRmsDb(output: string): number | null {
  const matches = [...output.matchAll(/RMS level dB:\s*(-?\d+(?:\.\d+)?)/g)];
  if (matches.length === 0) return null;
  const value = parseFloat(matches[matches.length - 1]![1]!);
  return Number.isFinite(value) ? value : null;
}

/** Runs ffmpeg with the given args and returns the last astats RMS-level reading, or null on failure. */
function measureRmsDb(ffmpegPath: string, args: string[]): number | null {
  const result = spawnSync(ffmpegPath, args, { encoding: 'utf8', timeout: MAKEUP_MEASURE_TIMEOUT_MS });
  if (result.error || result.signal) return null;
  return parseLastRmsDb(`${result.stdout ?? ''}${result.stderr ?? ''}`);
}

/**
 * Measures the makeup gain (dB) for this IR by convolving the reference signal
 * through the REAL production graph (at unity makeup) and comparing output RMS
 * to input RMS — running the actual graph, not a re-derived approximation,
 * guarantees the number matches what plays. Then subtracts MAKEUP_HEADROOM_DB so
 * the result sits a few dB BELOW a perfect level-match: hot masters land near the
 * limiter threshold instead of slamming ~3-4 dB into it. Clamped to [0, 30] dB,
 * rounded to 0.1 dB; falls back to DEFAULT_HRIR_MAKEUP_DB if either measurement
 * can't be read.
 */
export function measureHrirMakeupDb(ffmpegPath: string, filePath: string, format: HrirFormat): number {
  const inRms = measureRmsDb(ffmpegPath, [
    '-hide_banner', '-nostats',
    '-f', 'lavfi', '-i', MAKEUP_NOISE_LAVFI,
    '-af', 'astats=metadata=0', '-f', 'null', '-',
  ]);
  const outRms = measureRmsDb(ffmpegPath, [
    '-hide_banner', '-nostats',
    '-f', 'lavfi', '-i', MAKEUP_NOISE_LAVFI,
    '-i', filePath,
    '-filter_complex', buildHrirMeasureGraph(format),
    '-map', '[out]', '-f', 'null', '-',
  ]);
  if (inRms === null || outRms === null) {
    logger.warn(
      { filePath, format, inRms, outRms },
      'HRIR makeup-gain measurement failed - using the conservative default makeup for this file',
    );
    return DEFAULT_HRIR_MAKEUP_DB;
  }
  const makeup = Math.max(0, Math.min(30, Math.round((inRms - outRms - MAKEUP_HEADROOM_DB) * 10) / 10));
  return makeup;
}

export type HrirFormat = 'simple' | 'hesuvi14';

export interface HrirProfile {
  /** Stable id, persisted as the guild's applied profile — the filename without extension. */
  id: string;
  /** Absolute path to the WAV impulse-response file. */
  filePath: string;
  /**
   * 'simple': plain mono/stereo IR, used directly by audio/hrirFilterComplex.buildHrirFilterComplex.
   * 'hesuvi14': a genuine HeSuVi-style 14-channel HRIR (see audio/hrirFilterComplex.ts
   * for the exact channel mapping this was verified against).
   */
  format: HrirFormat;
  /**
   * Makeup gain (dB) that level-matches this IR's convolved output to the
   * pristine passthrough — measured against this exact file at load (see
   * measureHrirMakeupDb), NOT a hardcoded constant, so any bring-your-own BRIR
   * is auto-levelled. Baked into the filter chain by buildHrirFilterComplex.
   */
  makeupDb: number;
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
 * Scans a local directory for user-supplied HRIR/BRIR WAV files (bring-your-own —
 * see assets/hrir_profiles/README.md for why none are bundled). Missing directory
 * is not an error: the feature is simply unavailable until someone populates it.
 * EVERY file with a supported channel count is loaded as a selectable "Aura
 * Preset" (see the panel's preset select menu) — the guild picks which one is
 * applied and the choice persists (guildSettingsRepo.defaultHrirProfile). Each
 * file is probed and level-measured once at startup; the (alphabetical) first is
 * the default until a guild selects otherwise. Files with an unsupported channel
 * count are skipped with a warning rather than aborting the whole scan.
 */
export function loadHrirProfiles(ffmpegPath: string, dir: string = DEFAULT_HRIR_PROFILES_DIR): HrirProfile[] {
  let entries: string[];
  try {
    entries = readdirSync(dir);
  } catch {
    return [];
  }
  const wavFiles = entries.filter((f) => f.toLowerCase().endsWith('.wav')).sort();

  const loaded: HrirProfile[] = [];
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
    const makeupDb = measureHrirMakeupDb(ffmpegPath, filePath, format);
    logger.info({ id, format, makeupDb }, 'Measured HRIR makeup gain (level-matches the spatialised output to normal playback)');
    loaded.push({ id, filePath, format, makeupDb });
  }
  return loaded;
}
