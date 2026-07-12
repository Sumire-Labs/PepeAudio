import { existsSync } from 'node:fs';
import { logger } from '../logger.js';
import { getHrirCount, hasHrirCapacity } from './ffmpegConcurrencySlots.js';
import { buildHrirFallbackChain, buildAura360Chain, buildAura360Prefix } from './spatialFilterChain.js';
import { buildHrirFilterComplex } from './hrirFilterComplex.js';
import type { CreateTrackResourceParams } from './resourceTypes.js';
import type { HrirFormat } from '../config/hrirProfiles.js';
import { MAX_HRIR_CONCURRENCY } from '../player/constants.js';

/** The decided ffmpeg invocation shape once a non-fast-path spawn is needed — internal seam
 *  between deciding what to run and actually spawning/wiring it up (see ffmpegProcessLifecycle.ts). */
export interface FfmpegSpawnPlan {
  useHrir: boolean;
  /** Retired from the default path (raw HRTF was the muffle culprit); kept in the shape for the concurrency-slot wiring, always false. */
  useSofalizer: boolean;
  args: string[];
}

/** Internal-only plan shape returned by planFfmpegInvocation() - not exported from resourceFactory.ts. */
export type FfmpegPlan = { kind: 'fastPath' } | ({ kind: 'ffmpeg' } & FfmpegSpawnPlan);

function formatSeekSeconds(ms: number): string {
  return (ms / 1000).toFixed(3);
}

/**
 * Decides how to render a track:
 * - "Aura HRIR" is now a genuine TOGGLE. hrirMode 'off' means the untouched
 *   Opus fast path (the pristine reference), even when a BRIR file is present —
 *   the BRIR no longer forces always-on processing.
 * - hrirMode 'on' prefers the real BRIR virtualization (afir convolution,
 *   level-matched via the per-IR makeup gain) and falls back to the asset-free
 *   `-af` wide chain when no BRIR file is available or the convolution
 *   concurrency cap is hit. Both work on the guaranteed ffmpeg-static binary
 *   (no sofalizer/libmysofa needed — raw-HRTF sofalizer was retired for muffle).
 * - A non-zero seek in otherwise-normal mode still spawns ffmpeg for an `anull`
 *   reposition (e.g. a timestamped link, or resuming after a mid-track crash).
 */
export function planFfmpegInvocation(params: CreateTrackResourceParams): FfmpegPlan {
  const { hrirMode, aura360Mode, seekOffsetMs, hrirFilePath, hrirFormat, hrirMakeupDb } = params;

  const needsSeek = seekOffsetMs > 0;
  const hrirOn = hrirMode === 'on';
  const aura360On = aura360Mode === 'on';

  // Pristine fast path: both effects off and nothing else forcing an ffmpeg spawn.
  if (!hrirOn && !aura360On && !needsSeek) {
    return { kind: 'fastPath' };
  }

  // Aura HRIR ON: prefer real BRIR virtualization when a file is available and we're
  // under the convolution concurrency cap; otherwise the asset-free wide chain.
  let useHrir = false;
  if (hrirOn && hrirFilePath) {
    useHrir = existsSync(hrirFilePath);
    if (!useHrir) {
      logger.warn({ hrirFilePath }, 'HRIR profile file no longer exists on disk - falling back to the asset-free Aura HRIR chain');
    } else if (!hasHrirCapacity()) {
      logger.warn(
        { activeHrirCount: getHrirCount(), cap: MAX_HRIR_CONCURRENCY },
        'HRIR concurrency cap reached - falling back to the asset-free Aura HRIR chain for this stream',
      );
      useHrir = false;
    }
  }

  let args: string[];
  if (useHrir) {
    // afir's IR comes from a second real ffmpeg *input*, not a filter option
    // value — both -i's must precede -ss so it lands as an output-side seek
    // (input 0 is a non-seekable pipe, and placing -ss between the two -i's
    // would instead scope it as an *input* option to the IR file).
    const sec = formatSeekSeconds(seekOffsetMs);
    args = [
      '-loglevel', 'error',
      // Buffered temp file → fast input-side seek (`-ss` before `-i`); live pipe →
      // `-i pipe:0` with an output-side seek placed after BOTH inputs below.
      ...(params.seekableInput ? ['-ss', sec, '-i', params.seekableInput] : ['-i', 'pipe:0']),
      '-i', hrirFilePath as string,
      ...(!params.seekableInput && needsSeek ? ['-ss', sec] : []),
      '-filter_complex', buildHrirFilterComplex(hrirFormat as HrirFormat, hrirMakeupDb, aura360On ? buildAura360Prefix() : ''),
      '-map', '[out]',
      '-ac', '2',
      '-ar', '48000',
      '-f', 's16le',
      'pipe:1',
    ];
  } else {
    // Aura HRIR on with no BRIR (asset-free wide fallback), or Aura HRIR off but a
    // seek is needed ('anull' no-op reposition).
    // The Aura 360° effect (if on) and/or the asset-free Aura HRIR fallback (Aura
    // HRIR on but no BRIR file); 'anull' when we're only here to seek in
    // otherwise-normal mode.
    const chains: string[] = [];
    if (aura360On) chains.push(buildAura360Chain());
    if (hrirOn) chains.push(buildHrirFallbackChain());
    const filterChain = chains.length > 0 ? chains.join(',') : 'anull';
    const sec = formatSeekSeconds(seekOffsetMs);
    args = [
      '-loglevel', 'error',
      ...(params.seekableInput
        ? ['-ss', sec, '-i', params.seekableInput]
        : ['-i', 'pipe:0', ...(needsSeek ? ['-ss', sec] : [])]),
      '-af', filterChain,
      '-ac', '2',
      '-ar', '48000',
      '-f', 's16le',
      'pipe:1',
    ];
  }

  return { kind: 'ffmpeg', useHrir, useSofalizer: false, args };
}
