import type { Readable } from 'node:stream';
import { createFastPathResource } from './fastPathResource.js';
import { planFfmpegInvocation } from './ffmpegPlan.js';
import { spawnFfmpegResource } from './ffmpegProcessLifecycle.js';
import type { CreateTrackResourceParams, TrackResource } from './resourceTypes.js';
import { NORMAL_MODE_TRIM_FACTOR } from '../player/constants.js';

export type { CreateTrackResourceParams, TrackResource } from './resourceTypes.js';
export { destroyFfmpegProcess } from './ffmpegProcessLifecycle.js';

/**
 * Volume is NEVER an ffmpeg filter — always the inline volume transformer, so
 * the volume menu stays glitch-free regardless of Aura HRIR mode.
 */
export function createTrackResource(params: CreateTrackResourceParams): TrackResource {
  const plan = planFfmpegInvocation(params);
  // Normal mode is level-trimmed to match the quieter effect-on output, so toggling doesn't jump in volume; effect-on stays at unity.
  const normalTrim = params.hrirMode === 'off' && params.aura360Mode === 'off' ? NORMAL_MODE_TRIM_FACTOR : 1;
  if (plan.kind === 'fastPath') {
    // Fast path always has a live `stream` (a buffered reseek carries seekOffsetMs>0 and never resolves to fastPath), so the cast is safe.
    return createFastPathResource(params.stream as Readable, params.inputType, params.volumePercent, normalTrim);
  }
  return spawnFfmpegResource(plan, params, normalTrim);
}
