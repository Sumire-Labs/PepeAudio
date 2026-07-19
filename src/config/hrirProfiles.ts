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

const PROBE_TIMEOUT_MS = 10_000;

const MAKEUP_MEASURE_TIMEOUT_MS = 20_000;

/** Headroom (dB) below a perfect RMS level-match so hot masters don't ride continuously into the safety limiter (audible over-limiting/pumping). Tunable by ear. */
const MAKEUP_HEADROOM_DB = 4;

/** Fallback makeup if measurement fails (ffmpeg error/timeout/unparseable); deliberately conservative so it never over-boosts into the limiter. */
const DEFAULT_HRIR_MAKEUP_DB = 14;

/**
 * Decorrelated pink noise (two seeds joined to stereo, not mono-duplicated): a
 * correlated signal under-reads the BRIR level loss by ~2 dB. `join` needs an
 * explicit stereo layout so the downstream `pan=...|c0=FL...` can address channels
 * by name; lavfi rejects a trailing output pad label, so the graph ends unlabelled.
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

function measureRmsDb(ffmpegPath: string, args: string[]): number | null {
  const result = spawnSync(ffmpegPath, args, { encoding: 'utf8', timeout: MAKEUP_MEASURE_TIMEOUT_MS });
  if (result.error || result.signal) return null;
  return parseLastRmsDb(`${result.stdout ?? ''}${result.stderr ?? ''}`);
}

/**
 * Measures makeup gain (dB) by convolving the reference through the REAL production
 * graph at unity makeup and comparing output/input RMS — the actual graph, not a
 * re-derived approximation, so the number matches what plays. Falls back to
 * DEFAULT_HRIR_MAKEUP_DB if either measurement can't be read.
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
   * 'simple': plain mono/stereo IR, used directly. 'hesuvi14': genuine HeSuVi-style
   * 14-channel HRIR (see audio/hrirFilterComplex.ts for the channel mapping).
   */
  format: HrirFormat;
  /**
   * Makeup gain (dB) that level-matches this IR's convolved output to pristine
   * passthrough — measured per-file at load (measureHrirMakeupDb), not hardcoded,
   * so any bring-your-own BRIR is auto-levelled.
   */
  makeupDb: number;
}

/**
 * Reads the channel count via ffmpeg's `ashowinfo` `channels:N` field, NOT the
 * human-readable "Guessed Channel Layout" label — that's a display-only guess (any
 * 14-channel file gets called "9.1.4"). ffprobe would be more direct but isn't
 * bundled with our ffmpeg. Timeout-bounded because this runs synchronously at
 * startup with no watchdog; a stalled ffmpeg on a pathological file would hang the bot.
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
 * Classifies a probed channel count into a supported format, or null. 14 = HeSuVi's
 * standard "with reverb" HRIR shape (verified against real HeSuVi files). HeSuVi's
 * "no reverb" 7-channel layout is deliberately unsupported: a guessed mapping there
 * risks a silently-wrong (not crashing) result.
 */
function classifyFormat(channelCount: number | null): HrirFormat | null {
  if (channelCount === 1 || channelCount === 2) return 'simple';
  if (channelCount === 14) return 'hesuvi14';
  return null;
}

/**
 * Scans a local dir for user-supplied HRIR/BRIR WAVs (bring-your-own — see
 * assets/hrir_profiles/README.md). Missing directory is not an error. Every file
 * with a supported channel count is probed and level-measured once at startup and
 * offered as a preset; the alphabetical first is the default until a guild selects
 * otherwise. Unsupported channel counts are skipped with a warning, not aborting the scan.
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
