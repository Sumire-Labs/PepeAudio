import type { Readable } from 'node:stream';
import { createFastPathResource } from './fastPathResource.js';
import { planFfmpegInvocation } from './ffmpegPlan.js';
import { spawnFfmpegResource } from './ffmpegProcessLifecycle.js';
import type { CreateTrackResourceParams, TrackResource } from './resourceTypes.js';
import { NORMAL_MODE_TRIM_FACTOR } from '../player/constants.js';

export type { CreateTrackResourceParams, TrackResource } from './resourceTypes.js';
export { destroyFfmpegProcess } from './ffmpegProcessLifecycle.js';

/**
 * Builds the AudioResource for a track. Two paths, deliberately kept separate:
 * - fast path (hrirMode 'off' AND no seek needed): no ffmpeg process at all
 *   (createAudioResource's own internal transcoding, via @discordjs/voice/
 *   prism-media, is fine here since it never needs sofalizer). See
 *   fastPathResource.ts.
 * - ffmpeg path (hrirMode 'on', OR a non-zero seekOffsetMs even in normal
 *   mode — e.g. a link with a timestamp, or resuming after a mid-track crash):
 *   we spawn ffmpeg OURSELVES with the resolved sofalizer-capable binary (see
 *   ffmpegProcessLifecycle.ts for why this never goes through prism-media's
 *   `FFmpeg` class, and ffmpegPlan.ts for how the HRIR/sofalizer/seek decision
 *   and args[] are built).
 * Volume is NEVER an ffmpeg filter — it's always the inline volume transformer,
 * so the volume select menu stays glitch-free regardless of Aura HRIR mode.
 */
export function createTrackResource(params: CreateTrackResourceParams): TrackResource {
  const plan = planFfmpegInvocation(params);
  // Normal mode (Aura HRIR and Aura 360° both off) is level-trimmed to match the
  // quieter effect-on output, so toggling an effect doesn't jump in volume.
  // Effect-on paths stay at unity.
  const normalTrim = params.hrirMode === 'off' && params.aura360Mode === 'off' ? NORMAL_MODE_TRIM_FACTOR : 1;
  if (plan.kind === 'fastPath') {
    // Fast path only ever runs for a fresh, unbuffered, effect-free play, which
    // always has a live `stream` (a buffered reseek carries seekOffsetMs>0 and
    // never resolves to fastPath).
    return createFastPathResource(params.stream as Readable, params.inputType, params.volumePercent, normalTrim);
  }
  return spawnFfmpegResource(plan, params, normalTrim);
}
