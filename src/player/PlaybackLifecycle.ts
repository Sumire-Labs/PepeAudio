import type { ChildProcess } from 'node:child_process';
import type { Readable } from 'node:stream';
import { createWriteStream, existsSync, rm } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { randomUUID } from 'node:crypto';
import { type AudioPlayer, type AudioResource, type StreamType } from '@discordjs/voice';
import { createTrackResource, destroyFfmpegProcess } from '../audio/resourceFactory.js';
import type { QueueItem } from './QueueItem.js';
import type { AuraToggle } from './constants.js';
import { getHrirProfileById } from '../config/hrirProfilesState.js';
import { TRACK_BUFFER_PREFIX } from './trackBufferSweep.js';
import type { childLogger } from '../logger.js';
import type { FfmpegCapabilities } from '../config/ffmpegResolver.js';

export interface PlaybackLifecycleCallbacks {
  emitUpdate: () => void;
  isDestroyed: () => boolean;
  getVolume: () => number;
  getHrirMode: () => AuraToggle;
  getAura360Mode: () => AuraToggle;
  /** The guild's currently-selected Aura Preset (HRIR profile id), read fresh per track so a live preset switch takes effect on the next respawn. Null when no BRIR file is configured. */
  getHrirProfileId: () => string | null;
  setLastError: (message: string | null) => void;
  clearEmptyQueueTimer: () => void;
  /** Reads whatever's likely to play next (queue[0]) - see startTrack's prefetch call. */
  peekUpcoming: () => QueueItem | undefined;
}

/**
 * Owns the currently-playing track's resource/process lifecycle: starting,
 * tearing down, pausing/resuming, and reseeking (HRIR toggle, volume-
 * passthrough respawn, crash recovery). Never touches enqueueAction or holds
 * a GuildPlayer back-reference - everything it needs from GuildPlayer's own
 * state (volume, hrirMode, destroyed, lastError, emit('update')) comes
 * through the injected callbacks, so this class is structurally incapable of
 * re-entering the mutex.
 */
export class PlaybackLifecycle {
  currentResource: AudioResource | null = null;
  private activeFfmpegProcess: ChildProcess | null = null;
  private activeSourceStream: Readable | null = null;

  currentTrack: QueueItem | null = null;
  /** Whether HRIR processing is actually applied to the CURRENTLY playing resource (may be false even with an Aura Preset selected, e.g. if the file was deleted after startup). */
  usingHrir = false;
  /** Whether the CURRENT resource has an inline VolumeTransformer at all — false on the Opus-passthrough fast path (100% volume, nothing else needing it). See resourceFactory.ts. */
  private hasInlineVolume = false;

  playbackStartedAt: number | null = null;
  pausedAt: number | null = null;
  pausedTotalMs = 0;
  currentTrackRetryCount = 0;
  isRespawning = false;

  // Background full-speed buffer of the current track to a temp file — lets a
  // reseek (toggle/volume/crash) do a fast input-side seek instead of re-fetching.
  private currentTempFile: string | null = null;
  private tempFileComplete = false;
  private bufferSource: Readable | null = null;

  constructor(
    private readonly audioPlayer: AudioPlayer,
    private readonly ffmpeg: FfmpegCapabilities,
    private readonly log: ReturnType<typeof childLogger>,
    private readonly cb: PlaybackLifecycleCallbacks,
  ) {}

  // ---- elapsed-time bookkeeping (wall-clock based, survives respawns) ----
  getElapsedMs(): number {
    if (this.playbackStartedAt === null) return 0;
    const end = this.pausedAt ?? Date.now();
    return Math.max(0, end - this.playbackStartedAt - this.pausedTotalMs);
  }

  isPaused(): boolean {
    return this.pausedAt !== null;
  }

  /**
   * Kills the currently active ffmpeg process/source stream, if any, without
   * touching the AudioPlayer or currentResource — used right before starting a
   * DIFFERENT track (skip/previous/loop:track replay/reseek) so the outgoing
   * track's process is explicitly torn down rather than relying on
   * @discordjs/voice's AudioPlayer.play() destroying the old resource's
   * playStream and hoping ffmpeg exits on its own from the resulting broken pipe.
   * Safe to call even when there's nothing active (destroyFfmpegProcess/slot
   * release are idempotent).
   */
  teardownActiveResource(): void {
    if (this.activeFfmpegProcess) {
      destroyFfmpegProcess(this.activeFfmpegProcess, this.activeSourceStream ?? undefined);
    } else if (this.activeSourceStream) {
      this.activeSourceStream.destroy();
    }
    this.activeFfmpegProcess = null;
    this.activeSourceStream = null;
  }

  /** Stops the AudioPlayer and tears down any active ffmpeg process/source stream, whichever path is in use. */
  teardownPlayback(): void {
    this.teardownActiveResource();
    this.clearTrackBuffer();
    this.currentResource = null;
    this.audioPlayer.stop(true);
  }

  /**
   * Starts a SECOND, full-speed download of `track` into a temp file, decoupled
   * from playback pacing so it completes quickly. Once complete, reseeks (toggle/
   * volume/crash) input-seek that file instead of re-fetching + decode-discarding,
   * which is the whole point — snappy effect toggles. Purely best-effort: any
   * failure just leaves reseeks on the original re-fetch path. NOT torn down on a
   * reseek (only teardownActiveResource is), so the buffer keeps filling.
   */
  private startTrackBuffer(track: QueueItem): void {
    this.clearTrackBuffer();
    const tempFile = join(tmpdir(), `${TRACK_BUFFER_PREFIX}${randomUUID()}.webm`);
    this.currentTempFile = tempFile;
    track
      .getStream()
      .then(({ stream }) => {
        if (this.currentTempFile !== tempFile || this.cb.isDestroyed()) {
          stream.destroy();
          return;
        }
        this.bufferSource = stream;
        const ws = createWriteStream(tempFile);
        stream.pipe(ws);
        ws.once('finish', () => {
          if (this.currentTempFile === tempFile) this.tempFileComplete = true;
        });
        stream.once('error', (err) => this.log.debug({ err }, 'Track buffer download error (reseek re-fetches instead)'));
        ws.once('error', (err) => this.log.debug({ err }, 'Track buffer file write error'));
      })
      .catch((err) => this.log.debug({ err }, 'Track buffer getStream failed (reseek re-fetches instead)'));
  }

  /** Aborts any in-flight buffer download and deletes the temp file. */
  private clearTrackBuffer(): void {
    this.bufferSource?.destroy();
    this.bufferSource = null;
    this.tempFileComplete = false;
    const file = this.currentTempFile;
    this.currentTempFile = null;
    if (file) rm(file, { force: true }, () => undefined);
  }

  /**
   * `resetRetryCount` defaults to true (a genuinely new track starting fresh).
   * handlePlaybackFailureCore explicitly passes `false` when retrying the SAME
   * track in place via reseekCore — resetting on every startTrack call
   * (including in-place crash retries) previously meant a track that failed,
   * "succeeded" just long enough to reset the counter, then failed again
   * could retry indefinitely instead of respecting MAX_FFMPEG_CRASH_RETRIES.
   *
   * Public only so QueueHistoryManager (a sibling collaborator) can start
   * playback for a newly-picked track — this is INTERNAL USE ONLY for a
   * respawn: never call it directly to respawn the current track, use
   * reseekCore() instead (it's the only thing that correctly sets/clears
   * isRespawning around the call).
   */
  async startTrack(track: QueueItem, seekOffsetMs = 0, opts: { resetRetryCount?: boolean; fromBuffer?: boolean } = {}): Promise<void> {
    const resetRetryCount = opts.resetRetryCount ?? true;
    const fromBuffer = opts.fromBuffer ?? false;
    this.cb.clearEmptyQueueTimer();

    // Source selection: a COMPLETED background buffer of this same track lets a
    // reseek read the temp file with a fast input-side seek — no yt-dlp re-fetch,
    // no decode-and-discard. Otherwise fetch a fresh stream (and, on a genuinely
    // new track, kick off the background buffer for next time).
    const canUseBuffer =
      fromBuffer && this.tempFileComplete && this.currentTempFile !== null && existsSync(this.currentTempFile);

    let stream: Readable | undefined;
    let inputType: StreamType | undefined;
    let seekableInput: string | undefined;
    if (canUseBuffer) {
      seekableInput = this.currentTempFile ?? undefined;
    } else {
      const resolved = await track.getStream();
      stream = resolved.stream;
      inputType = resolved.inputType;
      if (this.cb.isDestroyed()) {
        // The player was torn down while we were awaiting the stream — discard
        // it rather than committing a resource to a dead voice connection.
        stream.destroy();
        throw new Error('GuildPlayer destroyed while resolving stream');
      }
      if (!fromBuffer) {
        this.startTrackBuffer(track);
      }
    }

    // getHrirProfileById reads a list cached once at startup, so this always
    // resolves whenever the selected id is non-null - it can't detect the
    // file being deleted mid-session. resourceFactory's existsSync check (and
    // its own warning log) is the real "does the file still exist" check;
    // created.usingHrir below reflects what ACTUALLY happened for this track.
    // Read fresh each track so a live Aura Preset switch applies on respawn.
    const profileId = this.cb.getHrirProfileId();
    const hrirProfile = profileId ? getHrirProfileById(profileId) : undefined;

    let created: ReturnType<typeof createTrackResource>;
    try {
      created = createTrackResource({
        stream,
        inputType,
        seekableInput,
        hrirMode: this.cb.getHrirMode(),
        aura360Mode: this.cb.getAura360Mode(),
        sofalizerAvailable: this.ffmpeg.sofalizerAvailable,
        ffmpegPath: this.ffmpeg.path,
        seekOffsetMs,
        volumePercent: this.cb.getVolume(),
        hrirFilePath: hrirProfile?.filePath ?? null,
        hrirFormat: hrirProfile?.format ?? null,
        hrirMakeupDb: hrirProfile?.makeupDb ?? 0,
      });
    } catch (err) {
      stream?.destroy();
      throw err;
    }
    const { resource, ffmpegProcess, usingHrir, hasInlineVolume } = created;

    this.currentResource = resource;
    this.activeFfmpegProcess = ffmpegProcess;
    this.activeSourceStream = stream ?? null;
    this.currentTrack = track;
    this.usingHrir = usingHrir;
    this.hasInlineVolume = hasInlineVolume;
    this.cb.setLastError(null);
    this.playbackStartedAt = Date.now() - seekOffsetMs;
    this.pausedAt = null;
    this.pausedTotalMs = 0;
    if (resetRetryCount) {
      this.currentTrackRetryCount = 0;
    }

    this.audioPlayer.play(resource);
    this.cb.emitUpdate();

    // Fire-and-forget warm-up for whatever's likely to play next (e.g. a lazily-
    // matched Spotify/Apple Music item's YouTube search - see youtubeMatch.ts).
    // Deliberately NOT routed through enqueueAction: prefetch never mutates
    // playback state, so chaining it onto the mutex would block the next real
    // skip/previous behind a network request for no reason. Wrong when shuffle
    // is on (queue[0] isn't necessarily next) but harmless either way - it
    // just means the cache warms for a track that doesn't end up playing next.
    this.cb.peekUpcoming()?.prefetch?.().catch(() => {});
  }

  pauseCore(): void {
    if (this.cb.isDestroyed() || this.isPaused()) return;
    this.audioPlayer.pause();
    this.pausedAt = Date.now();
    this.cb.emitUpdate();
  }

  resumeCore(): void {
    if (this.cb.isDestroyed() || !this.isPaused()) return;
    this.pausedTotalMs += Date.now() - (this.pausedAt ?? Date.now());
    this.pausedAt = null;
    this.audioPlayer.unpause();
    this.cb.emitUpdate();
  }

  /**
   * Shared by the HRIR toggle, the volume-passthrough respawn, and
   * ffmpeg crash recovery (and a future /seek command): kill the current
   * ffmpeg process/stream and restart the current track from `offsetMs`,
   * without misfiring the natural-track-end path. Callers must already be
   * running inside an enqueueAction'd `*Core` method on GuildPlayer.
   * `resetRetryCount` is forwarded to startTrack — crash recovery passes
   * `false` since it's retrying the SAME track in place (see startTrack's doc).
   */
  async reseekCore(offsetMs: number, opts: { resetRetryCount?: boolean } = {}): Promise<void> {
    if (!this.currentTrack) return;
    const track = this.currentTrack;
    this.isRespawning = true;
    try {
      this.teardownActiveResource();
      // fromBuffer: use the buffered temp file (fast input-seek) if it's ready.
      await this.startTrack(track, offsetMs, { ...opts, fromBuffer: true });
    } finally {
      this.isRespawning = false;
    }
  }

  /**
   * Mirrors setHrirModeCore's respawn/pause-preservation shape. Re-checks
   * its own preconditions (rather than trusting the caller's snapshot) since
   * this runs asynchronously behind the enqueueAction mutex - volume or track
   * may have changed again by the time it actually executes.
   */
  async applyVolumeRespawnCore(): Promise<void> {
    if (this.cb.isDestroyed() || !this.currentTrack || this.hasInlineVolume || this.cb.getVolume() === 100) return;
    const wasPaused = this.isPaused();
    const elapsed = this.getElapsedMs();
    await this.reseekCore(elapsed);
    if (wasPaused && !this.cb.isDestroyed() && this.currentTrack) {
      this.audioPlayer.pause();
      this.pausedAt = Date.now();
      this.cb.emitUpdate();
    }
  }
}
