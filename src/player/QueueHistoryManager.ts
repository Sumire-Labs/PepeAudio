import type { QueueItem } from './QueueItem.js';
import { AUTOPLAY_ENQUEUE_COUNT, MAX_HISTORY, MAX_QUEUE_LENGTH, type LoopMode } from './constants.js';
import type { childLogger } from '../logger.js';

function shuffleArray<T>(input: T[]): T[] {
  const copy = [...input];
  for (let i = copy.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    const a = copy[i]!;
    const b = copy[j]!;
    copy[i] = b;
    copy[j] = a;
  }
  return copy;
}

export interface QueueHistoryCallbacks {
  emitUpdate: () => void;
  isDestroyed: () => boolean;
  getCurrentTrack: () => QueueItem | null;
  startTrack: (track: QueueItem, seekOffsetMs?: number, opts?: { resetRetryCount?: boolean }) => Promise<void>;
  teardownActiveResource: () => void;
  teardownPlayback: () => void;
  startEmptyQueueTimer: () => void;
  /** Sets currentTrack to null only - see playNextCore's two distinct idle branches (one also clears playbackStartedAt, one doesn't). */
  clearCurrentTrack: () => void;
  clearPlaybackStartedAt: () => void;
  setLastError: (message: string | null) => void;
  /** Whether autoplay ("radio") is enabled for this guild - checked when the queue runs dry. */
  isAutoplayEnabled: () => boolean;
  /** Fetches related "radio" tracks seeded from the just-finished track (see sources/resolveAutoplayTracks). */
  fetchAutoplayTracks: (seed: QueueItem) => Promise<QueueItem[]>;
}

/**
 * Owns the queue/history/lapHistory/loopMode/shuffleEnabled state and the
 * playNextCore/previousCore logic that mutates it. Playback itself (starting
 * a track, tearing down ffmpeg) is delegated to PlaybackLifecycle via the
 * callbacks below - this class never touches enqueueAction or holds a
 * GuildPlayer back-reference, so it cannot re-enter the mutex.
 *
 * NOTE (deviation from the original file-split plan): the plan called for a
 * further pure/impure split of playNextCore into a separate queueAdvance.ts
 * decision-arithmetic module. That was deliberately skipped here - the two
 * "queue exhausted" branches below have a subtle, easy-to-lose asymmetry
 * (one clears playbackStartedAt, the other doesn't; one re-assigns `queue`,
 * the other doesn't), and preserving that exactly was judged more important
 * than hitting the file's line-count target. playNextCore/previousCore are
 * ported as single cohesive methods instead.
 */
export class QueueHistoryManager {
  queue: QueueItem[] = [];
  /** Capped at MAX_HISTORY — used only for the "previous" button/command. */
  history: QueueItem[] = [];
  /**
   * Uncapped, cleared whenever it's consumed to refill `queue` on a loop:'queue'
   * wraparound. Kept separate from `history` deliberately: `history` is capped
   * for the "previous" stack's bounded-memory purpose, but capping it ALSO
   * silently dropped the earliest tracks of a long queue once loop:'queue'
   * wrapped around (a confirmed data-loss bug) — this field exists solely to
   * reconstruct a full lap without that cap.
   */
  private lapHistory: QueueItem[] = [];

  loopMode: LoopMode = 'off';
  shuffleEnabled = false;

  constructor(
    private readonly log: ReturnType<typeof childLogger>,
    private readonly cb: QueueHistoryCallbacks,
  ) {}

  /** Returns how many items were actually enqueued (may be fewer than requested once MAX_QUEUE_LENGTH is hit). */
  enqueue(items: QueueItem[]): number {
    const capacity = Math.max(0, MAX_QUEUE_LENGTH - this.queue.length);
    if (capacity <= 0) {
      this.log.warn({ queueLength: this.queue.length, cap: MAX_QUEUE_LENGTH }, 'Queue is at capacity - dropping enqueue request');
      return 0;
    }
    const accepted = items.slice(0, capacity);
    this.queue.push(...accepted);
    if (accepted.length < items.length) {
      this.log.warn(
        { requested: items.length, accepted: accepted.length, cap: MAX_QUEUE_LENGTH },
        'Queue capacity reached - some items were not enqueued',
      );
    }
    return accepted.length;
  }

  /**
   * Removes a single PENDING queue item by its QueueItem.id (never the
   * currently-playing track, which lives in PlaybackLifecycle, not here).
   * Returns the removed item, or null if no queued item had that id. Used by
   * the web dashboard's queue-management UI. Callers route this through
   * GuildPlayer.enqueueAction so it can't interleave with playNextCore's
   * queue reassignment.
   */
  removeById(id: string): QueueItem | null {
    const index = this.queue.findIndex((item) => item.id === id);
    if (index === -1) return null;
    const [removed] = this.queue.splice(index, 1);
    return removed ?? null;
  }

  /**
   * Reorders a pending queue item to `toIndex` (clamped into range). Returns
   * false if no queued item had that id. Same serialization contract as
   * removeById.
   */
  moveById(id: string, toIndex: number): boolean {
    const from = this.queue.findIndex((item) => item.id === id);
    if (from === -1) return false;
    const clamped = Math.max(0, Math.min(this.queue.length - 1, Math.floor(toIndex)));
    if (from === clamped) return true;
    const [item] = this.queue.splice(from, 1);
    this.queue.splice(clamped, 0, item!);
    return true;
  }

  /** Clears only the PENDING queue (leaves history/lapHistory and the current track). Returns how many were removed. */
  clearQueue(): number {
    const count = this.queue.length;
    this.queue = [];
    return count;
  }

  /** Called from stopCore — clears queue/history/lapHistory together. */
  resetAll(): void {
    this.queue = [];
    this.history = [];
    this.lapHistory = [];
  }

  /**
   * Natural track-end / manual skip. `forceAdvance` bypasses loop:'track' replay
   * (manual skip always advances). State (queue/history/lapHistory) is only
   * committed AFTER a candidate track's stream has been confirmed playable —
   * a failed startTrack() no longer corrupts queue/history, and instead
   * recurses to try the next candidate. When there's truly nothing left to
   * play, this now actually stops the AudioPlayer/ffmpeg (previously it just
   * updated bookkeeping while the old track kept audibly playing).
   */
  async playNextCore(opts: { forceAdvance?: boolean } = {}): Promise<void> {
    if (this.cb.isDestroyed()) return;
    const forceAdvance = opts.forceAdvance ?? false;
    const currentTrack = this.cb.getCurrentTrack();

    if (!forceAdvance && this.loopMode === 'track' && currentTrack) {
      try {
        this.cb.teardownActiveResource();
        await this.cb.startTrack(currentTrack, 0);
      } catch (err) {
        this.log.error({ err, track: currentTrack?.title }, 'Failed to replay track for loop:track - skipping instead');
        await this.playNextCore({ forceAdvance: true });
      }
      return;
    }

    const finishedTrack = currentTrack;

    let candidateHistory = this.history;
    let candidateLapHistory = this.lapHistory;
    if (finishedTrack) {
      candidateHistory = [...this.history, finishedTrack];
      if (candidateHistory.length > MAX_HISTORY) {
        candidateHistory = candidateHistory.slice(candidateHistory.length - MAX_HISTORY);
      }
      candidateLapHistory = [...this.lapHistory, finishedTrack];
    }

    let candidateQueue = this.queue;
    let nextLapHistory = candidateLapHistory;
    if (candidateQueue.length === 0) {
      if (this.loopMode === 'queue' && candidateLapHistory.length > 0) {
        candidateQueue = this.shuffleEnabled ? shuffleArray(candidateLapHistory) : [...candidateLapHistory];
        nextLapHistory = [];
      } else {
        // Autoplay ("radio"): before giving up, try to keep the session going
        // with tracks related to what just finished. Only when it's enabled and
        // there's a finished track to seed from; a failure or an all-duplicates
        // result falls through to the normal stop below.
        const autoplayed =
          this.cb.isAutoplayEnabled() && finishedTrack
            ? await this.fetchFreshAutoplay(finishedTrack, candidateHistory, candidateLapHistory)
            : [];
        if (autoplayed.length > 0) {
          candidateQueue = autoplayed;
          nextLapHistory = candidateLapHistory;
        } else {
          this.history = candidateHistory;
          this.lapHistory = candidateLapHistory;
          this.cb.clearCurrentTrack();
          this.cb.clearPlaybackStartedAt();
          this.cb.teardownPlayback();
          this.cb.startEmptyQueueTimer();
          this.cb.emitUpdate();
          return;
        }
      }
    }

    const pickIndex = this.shuffleEnabled ? Math.floor(Math.random() * candidateQueue.length) : 0;
    const next = candidateQueue[pickIndex];
    if (!next) {
      this.history = candidateHistory;
      this.lapHistory = nextLapHistory;
      this.queue = candidateQueue;
      this.cb.clearCurrentTrack();
      this.cb.teardownPlayback();
      this.cb.emitUpdate();
      return;
    }
    const remainingQueue = [...candidateQueue.slice(0, pickIndex), ...candidateQueue.slice(pickIndex + 1)];

    // A link-supplied timestamp (e.g. YouTube's ?t=) applies once, on this
    // item's first play attempt — cleared here regardless of outcome so a
    // later loop:'track' repeat, loop:'queue' wraparound, or /previous back
    // to this same item plays from the top instead of jumping to the link's
    // timestamp every time.
    const startOffsetMs = next.initialOffsetMs ?? 0;
    next.initialOffsetMs = null;

    try {
      this.cb.teardownActiveResource();
      await this.cb.startTrack(next, startOffsetMs);
    } catch (err) {
      this.log.error({ err, track: next.title }, 'Failed to start the next track - skipping it');
      this.history = candidateHistory;
      this.lapHistory = nextLapHistory;
      this.queue = remainingQueue;
      await this.playNextCore({ forceAdvance: true });
      return;
    }

    this.history = candidateHistory;
    this.lapHistory = nextLapHistory;
    this.queue = remainingQueue;
  }

  /**
   * Fetches related "radio" tracks for autoplay and filters out anything already
   * played this session (the seed, history, and the current lap) so the radio
   * doesn't loop the same songs back-to-back. Returns at most
   * AUTOPLAY_ENQUEUE_COUNT items; an empty result (no relations found, fetch
   * failed, or all duplicates) tells playNextCore to stop as usual.
   */
  private async fetchFreshAutoplay(seed: QueueItem, history: QueueItem[], lapHistory: QueueItem[]): Promise<QueueItem[]> {
    let related: QueueItem[];
    try {
      related = await this.cb.fetchAutoplayTracks(seed);
    } catch (err) {
      this.log.warn({ err, seed: seed.title }, 'Autoplay: related-track fetch failed - stopping');
      return [];
    }
    const seen = new Set<string>([seed.sourceUrl, ...history.map((t) => t.sourceUrl), ...lapHistory.map((t) => t.sourceUrl)]);
    const fresh: QueueItem[] = [];
    for (const item of related) {
      if (seen.has(item.sourceUrl)) continue;
      seen.add(item.sourceUrl);
      fresh.push(item);
      if (fresh.length >= AUTOPLAY_ENQUEUE_COUNT) break;
    }
    if (fresh.length > 0) {
      this.log.info({ seed: seed.title, added: fresh.length }, 'Autoplay: continuing with related tracks');
    }
    return fresh;
  }

  /** Peeks (doesn't pop) history so a failed startTrack() leaves history/queue untouched. */
  async previousCore(): Promise<{ ok: boolean; reason?: string }> {
    if (this.cb.isDestroyed()) return { ok: false, reason: 'destroyed' };
    const prevTrack = this.history[this.history.length - 1];
    if (!prevTrack) return { ok: false, reason: 'no-history' };
    const outgoing = this.cb.getCurrentTrack();

    try {
      this.cb.teardownActiveResource();
      await this.cb.startTrack(prevTrack, 0);
    } catch (err) {
      this.log.error({ err, track: prevTrack.title }, 'Failed to start the previous track');
      this.cb.setLastError('前の曲の再生に失敗しました。');
      this.cb.emitUpdate();
      return { ok: false, reason: 'playback-failed' };
    }

    this.history.pop();
    if (outgoing) this.queue.unshift(outgoing);
    return { ok: true };
  }
}
