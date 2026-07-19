import type { Readable } from 'node:stream';
import type { ChildProcess } from 'node:child_process';
import type { StreamType, AudioResource } from '@discordjs/voice';
import type { AuraToggle } from '../player/constants.js';
import type { HrirFormat } from '../config/hrirProfiles.js';

export interface CreateTrackResourceParams {
  /** Live piped source for a fresh play; omitted for a buffered input-seek reseek (seekableInput is used instead). */
  stream?: Readable;
  inputType?: StreamType;
  /** When set, ffmpeg reads this temp file with a fast input-side seek (`-ss` before `-i`) instead of re-fetching + output-seeking — makes toggles snappy. */
  seekableInput?: string;
  hrirMode: AuraToggle;
  sofalizerAvailable: boolean;
  /** Absolute path to the resolved ffmpeg binary (see config/ffmpegResolver.ts). */
  ffmpegPath: string;
  /** Elapsed playback position to resume from (used on respawn/toggle/reseek). */
  seekOffsetMs: number;
  volumePercent: number;
  /** Takes priority over hrirMode/sofalizer when set — the two paths are mutually exclusive (layering both would double up spatialization). */
  hrirFilePath: string | null;
  /** The Aura 360° effect (widening + bass), independent of hrirMode/Aura HRIR. */
  aura360Mode: AuraToggle;
  /** Required alongside hrirFilePath — selects which filter_complex chain applies (see hrirFilterComplex.ts). */
  hrirFormat: HrirFormat | null;
  /** Per-IR makeup gain (dB), baked into the afir chain to level-match normal playback. */
  hrirMakeupDb: number;
}

export interface TrackResource {
  resource: AudioResource;
  /** Non-null only on the HRIR (ffmpeg) path — needed to tear it down for a respawn. */
  ffmpegProcess: ChildProcess | null;
  usingSofalizer: boolean;
  usingHrir: boolean;
  /** False on the Opus-passthrough fast path (100% volume, no ffmpeg/transcoding); otherwise true. */
  hasInlineVolume: boolean;
}
