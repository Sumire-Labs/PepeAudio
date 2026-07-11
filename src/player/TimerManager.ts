import { ALONE_TIMEOUT_MS, EMPTY_QUEUE_TIMEOUT_MS } from './constants.js';
import { upsertGuildSettings, type GuildSettings } from '../data/guildSettingsRepo.js';
import type { childLogger } from '../logger.js';

export interface TimerCallbacks {
  stop: () => Promise<void>;
}

/**
 * Owns the alone/empty-queue/settings-save timers plus the 24/7 (stay247)
 * flag that gates them. Never touches enqueueAction or holds a GuildPlayer
 * back-reference - the empty-queue timeout's eventual full-stop goes through
 * the injected `stop` callback (GuildPlayer's own public stop() method)
 * instead of calling back into GuildPlayer directly.
 */
export class TimerManager {
  stay247: boolean;
  private aloneTimer: NodeJS.Timeout | null = null;
  private emptyQueueTimer: NodeJS.Timeout | null = null;
  private settingsSaveTimer: NodeJS.Timeout | null = null;
  private pendingSettingsSave: Partial<Omit<GuildSettings, 'guildId' | 'updatedAt'>> | null = null;

  constructor(
    private readonly guildId: string,
    initialStay247: boolean,
    private readonly log: ReturnType<typeof childLogger>,
    private readonly cb: TimerCallbacks,
  ) {
    this.stay247 = initialStay247;
  }

  clearEmptyQueueTimer(): void {
    if (this.emptyQueueTimer) {
      clearTimeout(this.emptyQueueTimer);
      this.emptyQueueTimer = null;
    }
  }

  startEmptyQueueTimer(): void {
    this.clearEmptyQueueTimer();
    if (this.stay247) return; // 24/7 mode - never auto-disconnect on an empty queue
    this.emptyQueueTimer = setTimeout(() => {
      this.log.info('Queue empty timeout reached — disconnecting');
      void this.cb.stop().catch((err) => this.log.error({ err }, 'stop() failed from empty-queue timeout'));
    }, EMPTY_QUEUE_TIMEOUT_MS);
  }

  /** Started/cancelled by voiceStateUpdate.ts, which owns VC member-count checks. */
  startAloneTimer(onFire: () => void): void {
    this.cancelAloneTimer();
    if (this.stay247) return; // 24/7 mode - never auto-disconnect for being alone
    this.aloneTimer = setTimeout(onFire, ALONE_TIMEOUT_MS);
  }

  cancelAloneTimer(): void {
    if (this.aloneTimer) {
      clearTimeout(this.aloneTimer);
      this.aloneTimer = null;
    }
  }

  /** Toggles 24/7 mode. Enabling immediately cancels any already-running alone/empty-queue countdown. */
  setStay247(enabled: boolean): void {
    this.stay247 = enabled;
    if (enabled) {
      this.cancelAloneTimer();
      this.clearEmptyQueueTimer();
    }
    this.scheduleSettingsSave({ stay247: enabled });
  }

  scheduleSettingsSave(partial: Partial<Omit<GuildSettings, 'guildId' | 'updatedAt'>>): void {
    this.pendingSettingsSave = { ...this.pendingSettingsSave, ...partial };
    if (this.settingsSaveTimer) clearTimeout(this.settingsSaveTimer);
    this.settingsSaveTimer = setTimeout(() => {
      const toSave = this.pendingSettingsSave;
      this.pendingSettingsSave = null;
      this.settingsSaveTimer = null;
      if (toSave) upsertGuildSettings(this.guildId, toSave);
    }, 2_000);
  }

  /**
   * Called from stopCore — flushes (rather than discards) a pending debounced
   * save: a plain clearTimeout alone would silently drop a volume/3D-audio
   * change made within the last 2s.
   */
  flushPendingSettingsSave(): void {
    if (this.settingsSaveTimer) {
      clearTimeout(this.settingsSaveTimer);
      this.settingsSaveTimer = null;
      if (this.pendingSettingsSave) {
        upsertGuildSettings(this.guildId, this.pendingSettingsSave);
        this.pendingSettingsSave = null;
      }
    }
  }
}
