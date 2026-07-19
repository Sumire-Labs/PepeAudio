import { existsSync } from 'node:fs';
import { logger } from '../logger.js';
import { getHrirCount, hasHrirCapacity } from './ffmpegConcurrencySlots.js';
import { buildHrirFallbackChain, buildAura360Chain, buildAura360Prefix } from './spatialFilterChain.js';
import { buildHrirFilterComplex } from './hrirFilterComplex.js';
import type { CreateTrackResourceParams } from './resourceTypes.js';
import type { HrirFormat } from '../config/hrirProfiles.js';
import { MAX_HRIR_CONCURRENCY } from '../player/constants.js';

/** The decided ffmpeg invocation shape for a non-fast-path spawn. */
export interface FfmpegSpawnPlan {
  useHrir: boolean;
  /** Always false; kept for concurrency-slot wiring (raw-HRTF sofalizer was retired). */
  useSofalizer: boolean;
  args: string[];
}

export type FfmpegPlan = { kind: 'fastPath' } | ({ kind: 'ffmpeg' } & FfmpegSpawnPlan);

function formatSeekSeconds(ms: number): string {
  return (ms / 1000).toFixed(3);
}

/**
 * Decides fast-path (untouched Opus) vs an ffmpeg spawn. hrirMode 'off' stays on
 * the fast path even when a BRIR file exists — the file does not force processing.
 * hrirMode 'on' prefers the real BRIR (afir), falling back to the asset-free `-af`
 * wide chain when there is no BRIR file or the convolution concurrency cap is hit.
 * A non-zero seek alone also forces a spawn (anull reposition).
 */
export function planFfmpegInvocation(params: CreateTrackResourceParams): FfmpegPlan {
  const { hrirMode, aura360Mode, seekOffsetMs, hrirFilePath, hrirFormat, hrirMakeupDb } = params;

  const needsSeek = seekOffsetMs > 0;
  const hrirOn = hrirMode === 'on';
  const aura360On = aura360Mode === 'on';

  if (!hrirOn && !aura360On && !needsSeek) {
    return { kind: 'fastPath' };
  }

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
