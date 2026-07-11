import { createFastPathResource } from './fastPathResource.js';
import { planFfmpegInvocation } from './ffmpegPlan.js';
import { spawnFfmpegResource } from './ffmpegProcessLifecycle.js';
import type { CreateTrackResourceParams, TrackResource } from './resourceTypes.js';

export type { CreateTrackResourceParams, TrackResource } from './resourceTypes.js';
export { destroyFfmpegProcess } from './ffmpegProcessLifecycle.js';

/**
 * Builds the AudioResource for a track. Two paths, deliberately kept separate:
 * - fast path (spatialMode 'off' AND no seek needed): no ffmpeg process at all
 *   (createAudioResource's own internal transcoding, via @discordjs/voice/
 *   prism-media, is fine here since it never needs sofalizer). See
 *   fastPathResource.ts.
 * - ffmpeg path (spatialMode 'on', OR a non-zero seekOffsetMs even in normal
 *   mode — e.g. a link with a timestamp, or resuming after a mid-track crash):
 *   we spawn ffmpeg OURSELVES with the resolved sofalizer-capable binary (see
 *   ffmpegProcessLifecycle.ts for why this never goes through prism-media's
 *   `FFmpeg` class, and ffmpegPlan.ts for how the HRIR/sofalizer/seek decision
 *   and args[] are built).
 * Volume is NEVER an ffmpeg filter — it's always the inline volume transformer,
 * so the volume select menu stays glitch-free regardless of spatial mode.
 */
export function createTrackResource(params: CreateTrackResourceParams): TrackResource {
  const plan = planFfmpegInvocation(params);
  if (plan.kind === 'fastPath') {
    return createFastPathResource(params.stream, params.inputType, params.volumePercent);
  }
  return spawnFfmpegResource(plan, params);
}
