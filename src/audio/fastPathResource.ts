import type { Readable } from 'node:stream';
import { createAudioResource, StreamType } from '@discordjs/voice';
import type { TrackResource } from './resourceTypes.js';

/**
 * Fast path: no ffmpeg, no transcoding. At 100% volume with a WebM/Opus source,
 * @discordjs/voice demuxes the container without decoding/re-encoding Opus - the
 * single largest per-stream CPU cost. Any other volume needs inline PCM volume.
 */
export function createFastPathResource(
  stream: Readable,
  inputType: StreamType | undefined,
  volumePercent: number,
  trimFactor = 1,
): TrackResource {
  // A normal-mode trim (trimFactor < 1) needs inline volume even at 100%, so it
  // can't use the Opus-demux passthrough.
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
