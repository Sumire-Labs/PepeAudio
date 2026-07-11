import { existsSync } from 'node:fs';
import { logger } from '../logger.js';
import { getHrirCount, getSofalizerCount, hasHrirCapacity, hasSofalizerCapacity } from './ffmpegConcurrencySlots.js';
import { buildFilterChain, HRTF_SOFA_PATH } from './spatialFilterChain.js';
import { HRIR_HESUVI14_FILTER_COMPLEX, HRIR_SIMPLE_FILTER_COMPLEX } from './hrirFilterComplex.js';
import type { CreateTrackResourceParams } from './resourceTypes.js';
import { MAX_HRIR_CONCURRENCY, MAX_SOFALIZER_CONCURRENCY } from '../player/constants.js';

/** The decided ffmpeg invocation shape once a non-fast-path spawn is needed — internal seam
 *  between deciding what to run and actually spawning/wiring it up (see ffmpegProcessLifecycle.ts). */
export interface FfmpegSpawnPlan {
  useHrir: boolean;
  useSofalizer: boolean;
  args: string[];
}

/** Internal-only plan shape returned by planFfmpegInvocation() - not exported from resourceFactory.ts. */
export type FfmpegPlan = { kind: 'fastPath' } | ({ kind: 'ffmpeg' } & FfmpegSpawnPlan);

function formatSeekSeconds(ms: number): string {
  return (ms / 1000).toFixed(3);
}

export function planFfmpegInvocation(params: CreateTrackResourceParams): FfmpegPlan {
  const { spatialMode, sofalizerAvailable, seekOffsetMs, hrirFilePath, hrirFormat } = params;

  const needsSeek = seekOffsetMs > 0;
  if (spatialMode === 'off' && !needsSeek && !hrirFilePath) {
    return { kind: 'fastPath' };
  }

  let useHrir = false;
  if (hrirFilePath) {
    useHrir = existsSync(hrirFilePath);
    if (!useHrir) {
      logger.warn({ hrirFilePath }, 'HRIR profile file no longer exists on disk - playing without HRIR processing for this stream');
    } else if (!hasHrirCapacity()) {
      logger.warn(
        { activeHrirCount: getHrirCount(), cap: MAX_HRIR_CONCURRENCY },
        'HRIR concurrency cap reached - skipping HRIR processing for this stream',
      );
      useHrir = false;
    }
  }

  let useSofalizer = false;
  if (!useHrir && spatialMode === 'on') {
    useSofalizer = sofalizerAvailable && existsSync(HRTF_SOFA_PATH);
    if (useSofalizer && !hasSofalizerCapacity()) {
      logger.warn(
        { activeSofalizerCount: getSofalizerCount(), cap: MAX_SOFALIZER_CONCURRENCY },
        'Sofalizer concurrency cap reached - falling back to the lightweight spatial chain for this stream',
      );
      useSofalizer = false;
    }
  }

  // HRIR was configured but got skipped (file missing / concurrency cap), and
  // there's no sofalizer/seek reason to spawn ffmpeg either - fall back to the
  // fast path instead of spawning ffmpeg just to run a no-op `anull` filter.
  if (!useHrir && !useSofalizer && spatialMode === 'off' && !needsSeek) {
    return { kind: 'fastPath' };
  }

  let args: string[];
  if (useHrir) {
    // afir's IR comes from a second real ffmpeg *input*, not a filter option
    // value — both -i's must precede -ss so it lands as an output-side seek
    // (verified: input 0 is a non-seekable pipe, and placing -ss between the
    // two -i's would instead scope it as an *input* option to the IR file).
    args = [
      '-loglevel', 'error',
      '-i', 'pipe:0',
      '-i', hrirFilePath as string,
      ...(needsSeek ? ['-ss', formatSeekSeconds(seekOffsetMs)] : []),
      '-filter_complex', hrirFormat === 'hesuvi14' ? HRIR_HESUVI14_FILTER_COMPLEX : HRIR_SIMPLE_FILTER_COMPLEX,
      '-map', '[out]',
      '-ac', '2',
      '-ar', '48000',
      '-f', 's16le',
      'pipe:1',
    ];
  } else {
    // 'anull' (no-op) when we're only here to seek in otherwise-normal mode.
    const filterChain = spatialMode === 'on' ? buildFilterChain(useSofalizer) : 'anull';
    args = [
      '-loglevel', 'error',
      '-i', 'pipe:0',
      ...(needsSeek ? ['-ss', formatSeekSeconds(seekOffsetMs)] : []),
      '-af', filterChain,
      '-ac', '2',
      '-ar', '48000',
      '-f', 's16le',
      'pipe:1',
    ];
  }

  return { kind: 'ffmpeg', useHrir, useSofalizer, args };
}
