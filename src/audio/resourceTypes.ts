import type { Readable } from 'node:stream';
import type { ChildProcess } from 'node:child_process';
import type { StreamType, AudioResource } from '@discordjs/voice';
import type { SpatialMode } from '../player/constants.js';
import type { HrirFormat } from '../config/hrirProfiles.js';

export interface CreateTrackResourceParams {
  stream: Readable;
  inputType?: StreamType;
  spatialMode: SpatialMode;
  sofalizerAvailable: boolean;
  /** Absolute path to the resolved ffmpeg binary (see config/ffmpegResolver.ts). */
  ffmpegPath: string;
  /** Elapsed playback position to resume from (used on respawn/toggle/reseek). */
  seekOffsetMs: number;
  volumePercent: number;
  /**
   * Absolute path to a bring-your-own HRIR/BRIR WAV file (see config/hrirProfiles.ts),
   * or null. Takes priority over spatialMode/sofalizer when set - the two paths are
   * mutually exclusive (layering both would double up spatialization).
   */
  hrirFilePath: string | null;
  /** Required alongside hrirFilePath — selects which filter_complex chain applies (see hrirFilterComplex.ts). */
  hrirFormat: HrirFormat | null;
}

export interface TrackResource {
  resource: AudioResource;
  /** Non-null only on the spatial/HRIR (ffmpeg) path — needed to tear it down for a respawn. */
  ffmpegProcess: ChildProcess | null;
  usingSofalizer: boolean;
  usingHrir: boolean;
  /** Whether `resource` was created with `inlineVolume: true` — false on the Opus-passthrough fast path (100% volume, nothing else needing ffmpeg/transcoding). */
  hasInlineVolume: boolean;
}
