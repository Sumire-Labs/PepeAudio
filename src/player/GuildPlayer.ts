import { EventEmitter } from 'node:events';
import {
  type AudioPlayer,
  AudioPlayerStatus,
  createAudioPlayer,
  entersState,
  joinVoiceChannel,
  NoSubscriberBehavior,
  type VoiceConnection,
  VoiceConnectionStatus,
  type DiscordGatewayAdapterCreator,
} from '@discordjs/voice';
import type { QueueItem } from './QueueItem.js';
import {
  DEFAULT_VOLUME_PERCENT,
  MAX_FFMPEG_CRASH_RETRIES,
  MAX_VOLUME_PERCENT,
  type LoopMode,
  type PermissionMode,
  type SpatialMode,
} from './constants.js';
import { getGuildSettings, type GuildSettings } from '../data/guildSettingsRepo.js';
import { getHrirProfiles } from '../config/hrirProfilesState.js';
import { childLogger } from '../logger.js';
import type { FfmpegCapabilities } from '../config/ffmpegResolver.js';
import { attachConnectionRecovery } from './voiceConnectionRecovery.js';
import { PlaybackLifecycle } from './PlaybackLifecycle.js';
import { QueueHistoryManager } from './QueueHistoryManager.js';
import { TimerManager } from './TimerManager.js';

export interface GuildPlayerOptions {
  guildId: string;
  textChannelId: string;
  voiceChannelId: string;
  adapterCreator: DiscordGatewayAdapterCreator;
  ffmpeg: FfmpegCapabilities;
}

/**
 * Owns every piece of mutable playback state for one guild. Every command and
 * button handler reads/writes exclusively through a GuildPlayer instance
 * looked up via GuildPlayerManager — nothing else keeps its own copy.
 *
 * Emits 'update' after any state change a panel render should reflect, and
 * 'destroyed' exactly once, from stop().
 *
 * Every method that mutates playback state (skip/previous/stop/setSpatialMode,
 * plus the natural track-end and crash-recovery paths) runs through
 * `enqueueAction`, a per-instance promise chain that serializes them. Without
 * this, two overlapping calls (e.g. two different users pressing skip/previous
 * within the same cooldown window, or a natural track-end racing a manual
 * button press) could interleave around the `await track.getStream()` point
 * in startTrack() — this was a confirmed, reproducible bug (queue/history
 * corruption, leaked ffmpeg processes, and stop() racing an in-flight
 * operation to "resurrect" a destroyed player). The `*Core` methods below (and
 * the equivalent *Core methods on the collaborator classes this delegates to)
 * are the actual logic and must NEVER call enqueueAction themselves (that
 * would deadlock against their own still-pending outer call) — only the
 * public wrappers and the two AudioPlayer event listeners enqueue.
 *
 * As of the file-split refactor (see docs/file-split-refactor-plan.md phase
 * 5), most of that state/logic lives in three composed collaborators —
 * PlaybackLifecycle (current track/resource/timing, start/teardown/reseek),
 * QueueHistoryManager (queue/history/lapHistory, playNextCore/previousCore),
 * TimerManager (alone/empty-queue/settings-save timers + 24/7 mode) — plus a
 * small connection-recovery helper. Each collaborator is handed plain
 * callback closures for anything it needs from GuildPlayer's own state
 * (emitUpdate, isDestroyed, getVolume, etc.) and NEVER a reference back to
 * this class or to enqueueAction itself — that's a structural guarantee
 * against mutex re-entry, not just a naming convention. Cross-cutting logic
 * that spans more than one collaborator (stopCore, setSpatialModeCore,
 * handlePlaybackFailureCore) stays here.
 */
export class GuildPlayer extends EventEmitter {
  readonly guildId: string;
  textChannelId: string;
  voiceChannelId: string;
  readonly connection: VoiceConnection;
  readonly audioPlayer: AudioPlayer;

  private readonly log: ReturnType<typeof childLogger>;

  private readonly playback: PlaybackLifecycle;
  private readonly queueHistory: QueueHistoryManager;
  private readonly timers: TimerManager;

  spatialMode: SpatialMode;
  /**
   * HRIR profile id (see config/hrirProfiles.ts), or null if none is configured.
   * Fixed at construction to whatever's found in the HRIR folder - not user-selectable
   * (there used to be a per-guild toggle here; the panel select menu was removed and
   * this is now always "on" with the first/only available profile applied automatically).
   */
  readonly hrirProfile: string | null;
  volume: number;
  permissionMode: PermissionMode;
  djRoleId: string | null;
  lastError: string | null = null;
  destroyed = false;

  panelMessageId: string | null = null;
  panelChannelId: string | null = null;
  panelRefreshTimer: NodeJS.Timeout | null = null;

  /** Serializes all playback-mutating operations — see class doc comment. */
  private actionQueue: Promise<void> = Promise.resolve();

  constructor(opts: GuildPlayerOptions) {
    super();
    this.guildId = opts.guildId;
    this.textChannelId = opts.textChannelId;
    this.voiceChannelId = opts.voiceChannelId;
    this.log = childLogger({ guildId: this.guildId });

    const settings: GuildSettings = getGuildSettings(this.guildId);
    // Default volume is pinned to DEFAULT_VOLUME_PERCENT and 360° Sound is
    // always on (no user toggle) — both intentionally ignore the persisted
    // per-guild defaults.
    this.volume = DEFAULT_VOLUME_PERCENT;
    this.spatialMode = 'on';
    this.hrirProfile = getHrirProfiles()[0]?.id ?? null;
    this.permissionMode = settings.permissionMode;
    this.djRoleId = settings.djRoleId;

    this.connection = joinVoiceChannel({
      guildId: opts.guildId,
      channelId: opts.voiceChannelId,
      adapterCreator: opts.adapterCreator,
      selfDeaf: true,
    });
    attachConnectionRecovery(this.connection, this.log, () => this.stop());

    this.audioPlayer = createAudioPlayer({
      behaviors: { noSubscriber: NoSubscriberBehavior.Pause },
    });
    this.connection.subscribe(this.audioPlayer);

    this.playback = new PlaybackLifecycle(this.audioPlayer, opts.ffmpeg, this.hrirProfile, this.log, {
      emitUpdate: () => this.emit('update'),
      isDestroyed: () => this.destroyed,
      getVolume: () => this.volume,
      getSpatialMode: () => this.spatialMode,
      setLastError: (message) => {
        this.lastError = message;
      },
      clearEmptyQueueTimer: () => this.timers.clearEmptyQueueTimer(),
      peekUpcoming: () => this.queueHistory.queue[0],
    });

    this.queueHistory = new QueueHistoryManager(this.log, {
      emitUpdate: () => this.emit('update'),
      isDestroyed: () => this.destroyed,
      getCurrentTrack: () => this.playback.currentTrack,
      startTrack: (track, seekOffsetMs, o) => this.playback.startTrack(track, seekOffsetMs, o),
      teardownActiveResource: () => this.playback.teardownActiveResource(),
      teardownPlayback: () => this.playback.teardownPlayback(),
      startEmptyQueueTimer: () => this.timers.startEmptyQueueTimer(),
      clearCurrentTrack: () => {
        this.playback.currentTrack = null;
      },
      clearPlaybackStartedAt: () => {
        this.playback.playbackStartedAt = null;
      },
      setLastError: (message) => {
        this.lastError = message;
      },
    });

    this.timers = new TimerManager(this.guildId, settings.stay247, this.log, {
      stop: () => this.stop(),
    });

    this.audioPlayer.on('stateChange', (oldState, newState) => {
      this.log.debug(
        { from: oldState.status, to: newState.status, isRespawning: this.playback.isRespawning, elapsedMs: this.getElapsedMs() },
        'AudioPlayer state change',
      );
      if (newState.status === AudioPlayerStatus.Idle && oldState.status !== AudioPlayerStatus.Idle) {
        if (this.playback.isRespawning) return; // a deliberate respawn (toggle/reseek), not a real track end
        this.log.info(
          { track: this.currentTrack?.title, elapsedMs: this.getElapsedMs(), durationMs: this.currentTrack?.durationMs },
          'Track went idle - advancing to next',
        );
        void this.enqueueAction(() => this.queueHistory.playNextCore()).catch((err) =>
          this.log.error({ err }, 'playNext failed after track end'),
        );
      }
    });
    this.audioPlayer.on('error', (err) => {
      this.log.error({ err }, 'AudioPlayer error');
      // Set synchronously, BEFORE enqueueing: @discordjs/voice emits 'error'
      // then 'stateChange'->Idle in the same synchronous callstack for a
      // stream error. Without this, the stateChange listener above (which
      // fires second, nested inside this same callstack) reads isRespawning
      // as still false and enqueues a redundant playNextCore that runs after
      // recovery completes and incorrectly treats the just-recovered track as
      // finished, skipping it. Cleared once handlePlaybackFailureCore (which
      // itself may call reseekCore, toggling this again) fully settles.
      this.playback.isRespawning = true;
      void this.enqueueAction(() => this.handlePlaybackFailureCore())
        .catch((e) => this.log.error({ err: e }, 'handlePlaybackFailure failed'))
        .finally(() => {
          this.playback.isRespawning = false;
        });
    });
  }

  async waitUntilReady(timeoutMs = 30_000): Promise<void> {
    await entersState(this.connection, VoiceConnectionStatus.Ready, timeoutMs);
  }

  /**
   * Chains `action` onto this player's serialized action queue and returns its
   * result. The chain itself never rejects (a failed action doesn't wedge
   * subsequent ones) — callers still get the real rejection via the returned
   * promise. Never call this from inside a `*Core` method — only from a
   * public wrapper or an event listener — or it deadlocks against the outer
   * call that hasn't resolved yet.
   */
  private enqueueAction<T>(action: () => Promise<T>): Promise<T> {
    const run = this.actionQueue.then(() => action());
    this.actionQueue = run.then(
      () => undefined,
      () => undefined,
    );
    return run;
  }

  // ---- delegated read-only state (see PlaybackLifecycle/QueueHistoryManager) ----
  get currentTrack(): QueueItem | null {
    return this.playback.currentTrack;
  }

  get usingHrir(): boolean {
    return this.playback.usingHrir;
  }

  get queue(): QueueItem[] {
    return this.queueHistory.queue;
  }

  get history(): QueueItem[] {
    return this.queueHistory.history;
  }

  get loopMode(): LoopMode {
    return this.queueHistory.loopMode;
  }

  get shuffleEnabled(): boolean {
    return this.queueHistory.shuffleEnabled;
  }

  get stay247(): boolean {
    return this.timers.stay247;
  }

  // ---- elapsed-time bookkeeping (wall-clock based, survives respawns) ----
  getElapsedMs(): number {
    return this.playback.getElapsedMs();
  }

  isPaused(): boolean {
    return this.playback.isPaused();
  }

  get status(): 'idle' | 'playing' | 'paused' {
    if (!this.currentTrack) return 'idle';
    return this.isPaused() ? 'paused' : 'playing';
  }

  // ---- queue mutation ----
  /** Returns how many items were actually enqueued (may be fewer than requested once MAX_QUEUE_LENGTH is hit). */
  enqueue(items: QueueItem[]): number {
    return this.queueHistory.enqueue(items);
  }

  /** Started/cancelled by voiceStateUpdate.ts, which owns VC member-count checks. */
  startAloneTimer(onFire: () => void): void {
    this.timers.startAloneTimer(onFire);
  }

  cancelAloneTimer(): void {
    this.timers.cancelAloneTimer();
  }

  /** Toggles 24/7 mode. Enabling immediately cancels any already-running alone/empty-queue countdown. */
  setStay247(enabled: boolean): void {
    if (this.destroyed) return;
    this.timers.setStay247(enabled);
  }

  /**
   * Applies updated per-guild control-permission settings (from /settings) to
   * this live player so the change takes effect on the very next interaction,
   * not only after the player is next re-created. Pure in-memory gate state —
   * no effect on playback.
   */
  setPermissionSettings(permissionMode: PermissionMode, djRoleId: string | null): void {
    if (this.destroyed) return;
    this.permissionMode = permissionMode;
    this.djRoleId = djRoleId;
  }

  /** Public entry — used by play.command.ts to start initial playback, and internally by the natural-track-end listener. */
  async playNext(opts: { forceAdvance?: boolean } = {}): Promise<void> {
    await this.enqueueAction(() => this.queueHistory.playNextCore(opts));
  }

  async skip(): Promise<void> {
    await this.enqueueAction(() => this.queueHistory.playNextCore({ forceAdvance: true }));
  }

  async previous(): Promise<{ ok: boolean; reason?: string }> {
    return this.enqueueAction(() => this.queueHistory.previousCore());
  }

  /**
   * Routed through enqueueAction (unlike a plain synchronous setter) — without
   * this, a pause/resume landing while a skip/previous/track-transition is
   * mid-flight (awaiting track.getStream() inside startTrack) gets silently
   * discarded once that transition completes, since startTrack unconditionally
   * resets pausedAt to null and calls audioPlayer.play().
   */
  async pause(): Promise<void> {
    await this.enqueueAction(async () => this.playback.pauseCore());
  }

  async resume(): Promise<void> {
    await this.enqueueAction(async () => this.playback.resumeCore());
  }

  setVolume(percent: number): void {
    if (this.destroyed) return;
    this.volume = Math.max(0, Math.min(MAX_VOLUME_PERCENT, Math.round(percent)));
    this.emit('update');
    this.timers.scheduleSettingsSave({ defaultVolume: this.volume });

    if (this.playback.currentResource?.volume) {
      // Already has an inline VolumeTransformer (either a non-passthrough
      // resource, or a previous non-100% respawn already added one) - apply
      // directly, glitch-free, exactly as before.
      this.playback.currentResource.volume.setVolumeLogarithmic(this.volume / 100);
    } else if (this.playback.currentTrack && this.volume !== 100) {
      // Currently on the Opus-passthrough fast path (resourceFactory.ts) and
      // volume just moved away from 100% for the first time on this track -
      // the passthrough resource has no VolumeTransformer to adjust, so
      // respawn once to pick up an inline-volume resource. Bounded by
      // VOLUME_COOLDOWN_MS (500ms) on the panel select, so this can't be
      // spammed into repeated respawns.
      void this.enqueueAction(() => this.playback.applyVolumeRespawnCore()).catch((err) =>
        this.log.error({ err }, 'Failed to respawn for a non-default volume change'),
      );
    }
  }

  setLoopMode(mode: LoopMode): void {
    if (this.destroyed) return;
    this.queueHistory.loopMode = mode;
    this.emit('update');
  }

  toggleShuffle(): void {
    if (this.destroyed) return;
    this.queueHistory.shuffleEnabled = !this.queueHistory.shuffleEnabled;
    this.emit('update');
  }

  async setSpatialMode(mode: SpatialMode): Promise<void> {
    await this.enqueueAction(() => this.setSpatialModeCore(mode));
  }

  /** Preserves pause state across the toggle — previously, toggling while paused silently resumed playback. */
  private async setSpatialModeCore(mode: SpatialMode): Promise<void> {
    if (this.destroyed || mode === this.spatialMode) return;
    this.spatialMode = mode;
    this.emit('update'); // reflect the toggle before playback audio actually catches up
    this.timers.scheduleSettingsSave({ defaultSpatialMode: mode });
    if (!this.currentTrack) return;

    const wasPaused = this.isPaused();
    const elapsed = this.getElapsedMs();
    await this.playback.reseekCore(elapsed);
    if (wasPaused && !this.destroyed && this.currentTrack) {
      this.audioPlayer.pause();
      this.playback.pausedAt = Date.now();
      this.emit('update');
    }
  }

  private async handlePlaybackFailureCore(): Promise<void> {
    if (this.destroyed) return;
    if (this.playback.currentTrackRetryCount < MAX_FFMPEG_CRASH_RETRIES) {
      this.playback.currentTrackRetryCount += 1;
      this.log.warn({ retry: this.playback.currentTrackRetryCount }, 'Retrying current track after a playback failure');
      try {
        await this.playback.reseekCore(this.getElapsedMs(), { resetRetryCount: false });
      } catch (err) {
        this.log.error({ err }, 'Retry-via-reseek also failed - skipping to the next track');
        await this.queueHistory.playNextCore({ forceAdvance: true });
      }
      return;
    }
    this.log.error('Playback failed twice for this track - skipping to the next one');
    this.lastError = '再生エラーが発生したため、この曲をスキップしました。';
    this.emit('update');
    await this.queueHistory.playNextCore({ forceAdvance: true });
  }

  async stop(): Promise<void> {
    await this.enqueueAction(() => this.stopCore());
  }

  private async stopCore(): Promise<void> {
    if (this.destroyed) return;
    this.queueHistory.resetAll();
    this.playback.currentTrack = null;
    this.timers.cancelAloneTimer();
    this.timers.clearEmptyQueueTimer();
    if (this.panelRefreshTimer) {
      clearInterval(this.panelRefreshTimer);
      this.panelRefreshTimer = null;
    }
    // Flush rather than discard: a pending debounced save (e.g. a volume or
    // 3D-audio change within the last 2s) was previously silently dropped
    // here — flushPendingSettingsSave performs the write rather than just
    // cancelling the timer.
    this.timers.flushPendingSettingsSave();
    this.playback.teardownPlayback();
    this.connection.destroy();
    this.destroyed = true;
    this.emit('update');
    this.emit('destroyed');
  }
}
