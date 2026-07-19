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
  NORMAL_MODE_TRIM_FACTOR,
  AURA_ENABLED,
  type LoopMode,
  type PermissionMode,
  type AuraToggle,
} from './constants.js';
import { getGuildSettings, type GuildSettings } from '../data/guildSettingsRepo.js';
import { getHrirProfiles, getHrirProfileById } from '../config/hrirProfilesState.js';
import { childLogger } from '../logger.js';
import type { FfmpegCapabilities } from '../config/ffmpegResolver.js';
import { attachConnectionRecovery } from './voiceConnectionRecovery.js';
import { PlaybackLifecycle } from './PlaybackLifecycle.js';
import { QueueHistoryManager } from './QueueHistoryManager.js';
import { TimerManager } from './TimerManager.js';
import { resolveAutoplayTracks } from '../sources/index.js';

export interface GuildPlayerOptions {
  guildId: string;
  textChannelId: string;
  voiceChannelId: string;
  adapterCreator: DiscordGatewayAdapterCreator;
  ffmpeg: FfmpegCapabilities;
}

/**
 * Owns all mutable playback state for one guild; commands/buttons act only
 * through the GuildPlayer looked up via GuildPlayerManager.
 *
 * Every playback-state mutation runs through `enqueueAction`, a per-instance
 * promise chain that serializes them. Without it, overlapping calls interleave
 * around the `await track.getStream()` in startTrack(): queue/history
 * corruption, leaked ffmpeg processes, stop() racing an in-flight op to
 * resurrect a destroyed player. The `*Core` methods are the raw logic and must
 * NEVER call enqueueAction themselves (deadlock against their own still-pending
 * outer call) — only public wrappers and the two AudioPlayer listeners enqueue.
 *
 * Collaborators (PlaybackLifecycle, QueueHistoryManager, TimerManager) get plain
 * callback closures and NEVER a reference back to this class or enqueueAction —
 * a structural guard against mutex re-entry.
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

  hrirMode: AuraToggle;
  /** The Aura 360° effect (widening + bass), independent of hrirMode/Aura HRIR. */
  aura360Mode: AuraToggle;
  /** Selected Aura Preset — an HRIR profile id, or null if no BRIR file is configured. Only meaningful while Aura HRIR is on. */
  hrirProfile: string | null;
  volume: number;
  permissionMode: PermissionMode;
  djRoleId: string | null;
  /** Autoplay ("radio"): when the queue runs dry, pull related tracks instead of leaving. */
  autoplay: boolean;
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
    // Pinned to DEFAULT_VOLUME_PERCENT, ignoring the persisted per-guild volume.
    this.volume = DEFAULT_VOLUME_PERCENT;
    this.hrirMode = AURA_ENABLED ? settings.defaultHrirMode : 'off';
    this.aura360Mode = AURA_ENABLED ? settings.defaultAura360Mode : 'off';
    // Fall back if the persisted profile no longer resolves (files can be
    // added/removed between restarts); null when the folder is empty.
    const loadedProfiles = getHrirProfiles();
    const persistedProfile = settings.defaultHrirProfile;
    this.hrirProfile =
      persistedProfile && loadedProfiles.some((p) => p.id === persistedProfile)
        ? persistedProfile
        : (loadedProfiles[0]?.id ?? null);
    this.permissionMode = settings.permissionMode;
    this.djRoleId = settings.djRoleId;
    this.autoplay = settings.autoplay;

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

    this.playback = new PlaybackLifecycle(this.audioPlayer, opts.ffmpeg, this.log, {
      emitUpdate: () => this.emit('update'),
      isDestroyed: () => this.destroyed,
      getVolume: () => this.volume,
      getHrirMode: () => this.hrirMode,
      getAura360Mode: () => this.aura360Mode,
      getHrirProfileId: () => this.hrirProfile,
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
      isAutoplayEnabled: () => this.autoplay,
      fetchAutoplayTracks: (seed) => resolveAutoplayTracks(seed),
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
      // Set synchronously BEFORE enqueueing: @discordjs/voice emits 'error' then
      // 'stateChange'->Idle in the same synchronous callstack, so the stateChange
      // listener (firing second) would otherwise read isRespawning as false and
      // enqueue a redundant playNextCore that skips the just-recovered track.
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
   * Chains `action` onto the serialized action queue and returns its result. The
   * chain never rejects (a failed action doesn't wedge later ones); callers still
   * get the real rejection via the returned promise. Never call from inside a
   * `*Core` method — it deadlocks against the outer call that hasn't resolved yet.
   */
  private enqueueAction<T>(action: () => Promise<T>): Promise<T> {
    const run = this.actionQueue.then(() => action());
    this.actionQueue = run.then(
      () => undefined,
      () => undefined,
    );
    return run;
  }

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

  /** Applies /settings permission changes to this live player so they take effect on the next interaction. Pure in-memory gate state. */
  setPermissionSettings(permissionMode: PermissionMode, djRoleId: string | null): void {
    if (this.destroyed) return;
    this.permissionMode = permissionMode;
    this.djRoleId = djRoleId;
  }

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
   * Routed through enqueueAction so a pause/resume landing while a track
   * transition is mid-flight (awaiting track.getStream() inside startTrack) isn't
   * silently discarded — startTrack unconditionally resets pausedAt to null and
   * calls audioPlayer.play().
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
      // Already has an inline VolumeTransformer - apply directly, glitch-free.
      this.playback.currentResource.volume.setVolumeLogarithmic(this.volume / 100);
      // Normal mode is level-trimmed to match Aura 360°-on; setVolumeLogarithmic just
      // overwrote the transformer's linear gain, so re-apply the trim here.
      if (this.hrirMode === 'off') {
        const vol = this.playback.currentResource.volume;
        vol.setVolume(vol.volume * NORMAL_MODE_TRIM_FACTOR);
      }
    } else if (this.playback.currentTrack && this.volume !== 100) {
      // On the Opus-passthrough fast path (resourceFactory.ts) with no
      // VolumeTransformer to adjust, and volume just left 100% - respawn once to
      // pick up an inline-volume resource. Bounded by the panel's VOLUME_COOLDOWN_MS.
      void this.enqueueAction(() => this.playback.applyVolumeRespawnCore()).catch((err) =>
        this.log.error({ err }, 'Failed to respawn for a non-default volume change'),
      );
    }
  }

  setLoopMode(mode: LoopMode): void {
    if (this.destroyed) return;
    this.queueHistory.setLoopMode(mode);
    this.emit('update');
  }

  /**
   * Routed through enqueueAction (like skip/previous) so a removal can't
   * interleave with playNextCore's in-flight queue reassignment.
   */
  async removeQueueItem(id: string): Promise<boolean> {
    return this.enqueueAction(async () => {
      if (this.destroyed) return false;
      const removed = this.queueHistory.removeById(id);
      if (removed) this.emit('update');
      return Boolean(removed);
    });
  }

  /** Reorders a pending queue item to `toIndex`. Same serialization contract as removeQueueItem. */
  async moveQueueItem(id: string, toIndex: number): Promise<boolean> {
    return this.enqueueAction(async () => {
      if (this.destroyed) return false;
      const ok = this.queueHistory.moveById(id, toIndex);
      if (ok) this.emit('update');
      return ok;
    });
  }

  /**
   * Seeks the current track to `positionMs` (clamped), preserving pause state.
   * Best-effort: seeking within the buffered range is instant; a far-forward
   * seek re-fetches from the source.
   */
  async seek(positionMs: number): Promise<void> {
    await this.enqueueAction(() => this.seekCore(positionMs));
  }

  private async seekCore(positionMs: number): Promise<void> {
    if (this.destroyed || !this.currentTrack) return;
    const durationMs = this.currentTrack.durationMs;
    // Keep a small tail so seeking to the very end doesn't instantly advance.
    const upper = durationMs && durationMs > 1000 ? durationMs - 1000 : Number.MAX_SAFE_INTEGER;
    const clamped = Math.max(0, Math.min(upper, Math.floor(positionMs)));
    const wasPaused = this.isPaused();
    await this.playback.reseekCore(clamped);
    if (wasPaused && !this.destroyed && this.currentTrack) {
      this.audioPlayer.pause();
      this.playback.pausedAt = Date.now();
      this.emit('update');
    }
  }

  async jumpToQueueItem(id: string): Promise<boolean> {
    return this.enqueueAction(() => this.queueHistory.jumpToCore(id));
  }

  /** Clears the pending queue (not the current track). Returns how many items were removed. */
  async clearQueue(): Promise<number> {
    return this.enqueueAction(async () => {
      if (this.destroyed) return 0;
      const count = this.queueHistory.clearQueue();
      if (count > 0) this.emit('update');
      return count;
    });
  }

  toggleShuffle(): void {
    if (this.destroyed) return;
    this.queueHistory.setShuffle(!this.shuffleEnabled);
    this.emit('update');
  }

  /** Toggles autoplay ("radio") and persists it. Only takes effect the next time the queue runs dry, so no respawn is needed. */
  setAutoplay(enabled: boolean): void {
    if (this.destroyed || enabled === this.autoplay) return;
    this.autoplay = enabled;
    this.timers.scheduleSettingsSave({ autoplay: enabled });
    this.emit('update');
  }

  async setHrirMode(mode: AuraToggle): Promise<void> {
    await this.enqueueAction(() => this.setHrirModeCore(mode));
  }

  /** Preserves pause state across the toggle — previously, toggling while paused silently resumed playback. */
  private async setHrirModeCore(mode: AuraToggle): Promise<void> {
    if (this.destroyed || mode === this.hrirMode) return;
    this.hrirMode = mode;
    this.emit('update'); // reflect the toggle before playback audio actually catches up
    this.timers.scheduleSettingsSave({ defaultHrirMode: mode });
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

  async setAura360Mode(mode: AuraToggle): Promise<void> {
    await this.enqueueAction(() => this.setAura360ModeCore(mode));
  }

  /** Respawns the current track so the Aura 360° toggle takes effect, preserving pause state. */
  private async setAura360ModeCore(mode: AuraToggle): Promise<void> {
    if (this.destroyed || mode === this.aura360Mode) return;
    this.aura360Mode = mode;
    this.emit('update');
    this.timers.scheduleSettingsSave({ defaultAura360Mode: mode });
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

  async setAuraPreset(id: string): Promise<void> {
    await this.enqueueAction(() => this.setAuraPresetCore(id));
  }

  /**
   * Switches the applied Aura Preset (BRIR/HRIR impulse response) and persists it.
   * Respawns the current track ONLY when Aura HRIR is on; while off, just records
   * the selection (it applies when Aura HRIR is next turned on). Ignores unknown ids.
   */
  private async setAuraPresetCore(id: string): Promise<void> {
    if (this.destroyed || id === this.hrirProfile || !getHrirProfileById(id)) return;
    this.hrirProfile = id;
    this.emit('update');
    this.timers.scheduleSettingsSave({ defaultHrirProfile: id });
    if (this.hrirMode !== 'on' || !this.currentTrack) return;

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
    // Flush rather than discard: a pending debounced save (volume/3D-audio change
    // in the last 2s) was previously dropped here; flush performs the write.
    this.timers.flushPendingSettingsSave();
    this.playback.teardownPlayback();
    this.connection.destroy();
    this.destroyed = true;
    this.emit('update');
    this.emit('destroyed');
  }
}
