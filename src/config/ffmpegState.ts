import type { FfmpegCapabilities } from './ffmpegResolver.js';

let capabilities: FfmpegCapabilities | null = null;

export function setFfmpegCapabilities(caps: FfmpegCapabilities): void {
  capabilities = caps;
}

export function getFfmpegCapabilities(): FfmpegCapabilities {
  if (!capabilities) {
    throw new Error('ffmpeg capabilities not initialized — call initFfmpeg() at startup first');
  }
  return capabilities;
}
