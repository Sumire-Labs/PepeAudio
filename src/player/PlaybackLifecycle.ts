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
  /** Read fresh per track so a live Aura Preset switch applies on the next respawn. Null when no BRIR file is configured. */
  getHrirProfileId: () => string | null;
  setLastError: (message: string | null) => void;
  clearEmptyQueueTimer: () => void;
  /** Whatever's likely to play next (queue[0]). */
  peekUpcoming: () => QueueItem | undefined;
}

/**
 * Owns the currently-playing track's resource/process lifecycle. Reaches
 * GuildPlayer state only through injected callbacks (never a back-reference),
 * so it is structurally incapable of re-entering the enqueueAction mutex.
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

  // Background full-speed buffer of the current track for fast reseeks.
  private currentTempFile: string | null = null;
  private tempFileComplete = false;
  private bufferSource: Readable | null = null;

  constructor(
    private readonly audioPlayer: AudioPlayer,
    private readonly ffmpeg: FfmpegCapabilities,
    private readonly log: ReturnType<typeof childLogger>,
    private readonly cb: PlaybackLifecycleCallbacks,
  ) {}

  getElapsedMs(): number {
    if (this.playbackStartedAt === null) return 0;
    const end = this.pausedAt ?? Date.now();
    return Math.max(0, end - this.playbackStartedAt - this.pausedTotalMs);
  }

  isPaused(): boolean {
    return this.pausedAt !== null;
  }

  /**
   * Kills the active ffmpeg process/source stream without touching the
   * AudioPlayer or currentResource — used right before starting a DIFFERENT
   * track so the outgoing process is explicitly torn down instead of relying on
   * AudioPlayer.play() breaking the pipe. Idempotent; safe when nothing active.
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

  teardownPlayback(): void {
    this.teardownActiveResource();
    this.clearTrackBuffer();
    this.currentResource = null;
    this.audioPlayer.stop(true);
  }

  /**
   * Starts a SECOND, full-speed download of `track` into a temp file so a later
   * reseek can input-seek it instead of re-fetching. Best-effort: any failure
   * just leaves reseeks on the re-fetch path. NOT torn down on a reseek (only
   * teardownActiveResource is), so the buffer keeps filling.
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

  private clearTrackBuffer(): void {
    this.bufferSource?.destroy();
    this.bufferSource = null;
    this.tempFileComplete = false;
    const file = this.currentTempFile;
    this.currentTempFile = null;
    if (file) rm(file, { force: true }, () => undefined);
  }

  /**
   * `resetRetryCount` defaults to true (new track starting fresh).
   * handlePlaybackFailureCore passes `false` when retrying the SAME track in
   * place via reseekCore — otherwise a track that "succeeds" just long enough
   * to reset the counter, then fails again, could retry past
   * MAX_FFMPEG_CRASH_RETRIES forever.
   *
   * Public only so QueueHistoryManager can start a newly-picked track. To
   * respawn the CURRENT track never call this directly — use reseekCore(), the
   * only path that correctly sets/clears isRespawning around the call.
   */
  async startTrack(track: QueueItem, seekOffsetMs = 0, opts: { resetRetryCount?: boolean; fromBuffer?: boolean } = {}): Promise<void> {
    const resetRetryCount = opts.resetRetryCount ?? true;
    const fromBuffer = opts.fromBuffer ?? false;
    this.cb.clearEmptyQueueTimer();

    // A COMPLETED background buffer lets a reseek input-seek the temp file (no
    // re-fetch/decode-discard); otherwise fetch fresh and kick off the buffer.
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
        // Torn down while awaiting the stream — discard it, don't commit a resource to a dead connection.
        stream.destroy();
        throw new Error('GuildPlayer destroyed while resolving stream');
      }
      if (!fromBuffer) {
        this.startTrackBuffer(track);
      }
    }

    // getHrirProfileById reads a startup-cached list, so it resolves whenever
    // the id is non-null and can't detect the file being deleted mid-session —
    // resourceFactory's existsSync is the real check, and created.usingHrir
    // below reflects what actually happened. Read fresh each track so a live
    // Aura Preset switch applies on respawn.
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

    // Fire-and-forget warm-up for whatever's likely to play next. Deliberately
    // NOT routed through enqueueAction: prefetch never mutates playback state, so
    // chaining it onto the mutex would block the next real skip behind a network
    // request. Wrong under shuffle (queue[0] isn't necessarily next) but harmless.
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
   * Shared by HRIR toggle, volume-passthrough respawn, seek, and ffmpeg crash
   * recovery: kill the current process/stream and restart the current track from
   * `offsetMs` without misfiring the natural-track-end path. Callers must already
   * be inside an enqueueAction'd `*Core` method. `resetRetryCount` is forwarded
   * to startTrack (crash recovery passes false — same track retried in place).
   */
  async reseekCore(offsetMs: number, opts: { resetRetryCount?: boolean } = {}): Promise<void> {
    if (!this.currentTrack) return;
    const track = this.currentTrack;
    this.isRespawning = true;
    try {
      this.teardownActiveResource();
      await this.startTrack(track, offsetMs, { ...opts, fromBuffer: true });
    } finally {
      this.isRespawning = false;
    }
  }

  /**
   * Re-checks its own preconditions rather than trusting the caller's snapshot:
   * it runs asynchronously behind the enqueueAction mutex, so volume or track
   * may have changed again by the time it executes.
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
