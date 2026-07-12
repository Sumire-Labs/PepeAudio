import type { Readable } from 'node:stream';
import { createAudioResource, StreamType } from '@discordjs/voice';
import type { TrackResource } from './resourceTypes.js';

/**
 * Fast path: no ffmpeg process, no transcoding. `inlineVolume` (prism-media's
 * VolumeTransformer) is itself skipped when volumePercent is 100 - for a
 * WebM/Opus source (yt-dlp's always-itag-251 audio, per youtube.ts) this means
 * @discordjs/voice only demuxes the container and never decodes/re-encodes
 * Opus at all, which is the single largest per-stream CPU cost in the whole
 * pipeline (see docs/performance-optimization-plan.md phase 2). At any other
 * volume, inline volume is required exactly as before (it operates on raw PCM
 * gain, not something a demux-only passthrough could apply).
 */
export function createFastPathResource(
  stream: Readable,
  inputType: StreamType | undefined,
  volumePercent: number,
  trimFactor = 1,
): TrackResource {
  // A normal-mode trim (trimFactor < 1, see resourceFactory) needs the inline
  // VolumeTransformer even at 100% volume, so it can't stay on the pure
  // Opus-demux passthrough in that case.
  const useInlineVolume = volumePercent !== 100 || trimFactor !== 1;
  const resource = createAudioResource(stream, {
    inputType: inputType ?? StreamType.Arbitrary,
    inlineVolume: useInlineVolume,
  });
  if (useInlineVolume) {
    resource.volume?.setVolumeLogarithmic(volumePercent / 100);
    if (resource.volume && trimFactor !== 1) {
      resource.volume.setVolume(resource.volume.volume * trimFactor);
    }
  }
  return { resource, ffmpegProcess: null, usingSofalizer: false, usingHrir: false, hasInlineVolume: useInlineVolume };
}
