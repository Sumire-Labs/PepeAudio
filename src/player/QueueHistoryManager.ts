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

/** In-place shuffle: the queue's displayed order IS the play order. */
function shuffleInPlace<T>(arr: T[]): void {
  for (let i = arr.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    const a = arr[i]!;
    arr[i] = arr[j]!;
    arr[j] = a;
  }
}

export interface QueueHistoryCallbacks {
  emitUpdate: () => void;
  isDestroyed: () => boolean;
  getCurrentTrack: () => QueueItem | null;
  startTrack: (track: QueueItem, seekOffsetMs?: number, opts?: { resetRetryCount?: boolean }) => Promise<void>;
  teardownActiveResource: () => void;
  teardownPlayback: () => void;
  startEmptyQueueTimer: () => void;
  /** Clears currentTrack only, not playbackStartedAt (they clear independently). */
  clearCurrentTrack: () => void;
  clearPlaybackStartedAt: () => void;
  setLastError: (message: string | null) => void;
  isAutoplayEnabled: () => boolean;
  fetchAutoplayTracks: (seed: QueueItem) => Promise<QueueItem[]>;
}

/**
 * Never holds a GuildPlayer back-reference and never calls enqueueAction, so it
 * cannot re-enter the mutex.
 *
 * The two "queue exhausted" branches in playNextCore have a subtle asymmetry
 * (one clears playbackStartedAt, the other doesn't; one re-assigns `queue`, the
 * other doesn't) — preserve it exactly.
 */
export class QueueHistoryManager {
  queue: QueueItem[] = [];
  /** Capped at MAX_HISTORY — used only for the "previous" button/command. */
  history: QueueItem[] = [];
  /**
   * Uncapped loop-rotation buffer, cleared when consumed to refill `queue` on a
   * loop:'queue' wraparound. Kept separate from the capped `history`: capping it
   * dropped the earliest tracks on wraparound (a data-loss bug).
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
    if (this.shuffleEnabled) {
      // Interleave at random positions so the displayed queue order stays the play order.
      for (const item of accepted) {
        this.queue.splice(Math.floor(Math.random() * (this.queue.length + 1)), 0, item);
      }
    } else {
      this.queue.push(...accepted);
    }
    if (accepted.length < items.length) {
      this.log.warn(
        { requested: items.length, accepted: accepted.length, cap: MAX_QUEUE_LENGTH },
        'Queue capacity reached - some items were not enqueued',
      );
    }
    return accepted.length;
  }

  /**
   * Removes a single PENDING queue item by id (never the currently-playing
   * track, which lives in PlaybackLifecycle). Returns the removed item, or null
   * if none matched. Callers route this through GuildPlayer.enqueueAction so it
   * can't interleave with playNextCore's queue reassignment.
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

  /**
   * Jumps to a queued item by id: starts it now, discards the queued items
   * before it, keeps the ones after. Deterministic regardless of shuffle. Starts
   * first and commits queue/history only on success, so a failed start leaves
   * them untouched. Returns false if no queued item has that id.
   */
  async jumpToCore(id: string): Promise<boolean> {
    if (this.cb.isDestroyed()) return false;
    const index = this.queue.findIndex((item) => item.id === id);
    if (index === -1) return false;
    const target = this.queue[index]!;
    const outgoing = this.cb.getCurrentTrack();
    const remaining = this.queue.slice(index + 1);

    const startOffsetMs = target.initialOffsetMs ?? 0;
    target.initialOffsetMs = null;
    try {
      this.cb.teardownActiveResource();
      await this.cb.startTrack(target, startOffsetMs);
    } catch (err) {
      this.log.error({ err, track: target.title }, 'Failed to start the jumped-to track');
      return false;
    }

    if (outgoing) {
      const nextHistory = [...this.history, outgoing];
      this.history = nextHistory.length > MAX_HISTORY ? nextHistory.slice(nextHistory.length - MAX_HISTORY) : nextHistory;
      // Keep the loop rotation only while queue-loop is active.
      if (this.loopMode === 'queue') this.lapHistory = [...this.lapHistory, outgoing];
    }
    this.queue = remaining;
    this.cb.emitUpdate();
    return true;
  }

  /** Clears only the PENDING queue (leaves history/lapHistory and the current track). Returns how many were removed. */
  clearQueue(): number {
    const count = this.queue.length;
    this.queue = [];
    return count;
  }

  /** Clears queue, history, and lapHistory together. */
  resetAll(): void {
    this.queue = [];
    this.history = [];
    this.lapHistory = [];
  }

  /**
   * Switching TO 'queue' re-arms the loop rotation by clearing lapHistory, so
   * the loop set becomes the current track + what's queued + anything added
   * after — NOT songs already played before the loop was enabled.
   */
  setLoopMode(mode: LoopMode): void {
    if (mode === 'queue' && this.loopMode !== 'queue') this.lapHistory = [];
    this.loopMode = mode;
  }

  /**
   * Enabling shuffles the current pending queue in place so its displayed order
   * equals the play order (playNextCore always plays from the front); disabling
   * leaves the order as-is (pre-shuffle order can't be restored).
   */
  setShuffle(enabled: boolean): void {
    if (enabled && !this.shuffleEnabled) shuffleInPlace(this.queue);
    this.shuffleEnabled = enabled;
  }

  /**
   * Natural track-end / manual skip. `forceAdvance` bypasses loop:'track' replay
   * (manual skip always advances). queue/history/lapHistory are committed only
   * AFTER a candidate's stream is confirmed playable, so a failed startTrack()
   * doesn't corrupt them; it recurses to try the next candidate.
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
      // Only build the loop rotation while queue-loop is active — otherwise
      // lapHistory accumulates across the whole session and is never freed.
      candidateLapHistory = this.loopMode === 'queue' ? [...this.lapHistory, finishedTrack] : this.lapHistory;
    }

    let candidateQueue = this.queue;
    let nextLapHistory = candidateLapHistory;
    if (candidateQueue.length === 0) {
      if (this.loopMode === 'queue' && candidateLapHistory.length > 0) {
        candidateQueue = this.shuffleEnabled ? shuffleArray(candidateLapHistory) : [...candidateLapHistory];
        nextLapHistory = [];
      } else {
        // Autoplay ("radio"): before giving up, try tracks related to what just
        // finished. A failure or all-duplicates result falls through to the stop below.
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

    // Always play from the front: shuffle reorders the queue itself, so the head
    // IS the correct next track.
    const next = candidateQueue[0];
    if (!next) {
      this.history = candidateHistory;
      this.lapHistory = nextLapHistory;
      this.queue = candidateQueue;
      this.cb.clearCurrentTrack();
      this.cb.teardownPlayback();
      this.cb.emitUpdate();
      return;
    }
    const remainingQueue = candidateQueue.slice(1);

    // A link-supplied timestamp (e.g. YouTube's ?t=) applies once, on first
    // play — cleared here regardless of outcome so a later loop:'track' repeat,
    // loop:'queue' wraparound, or /previous back to this item starts from the top.
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
   * Fetches related "radio" tracks and filters out anything already played this
   * session (seed, history, current lap) so the radio doesn't loop the same
   * songs back-to-back. Returns at most AUTOPLAY_ENQUEUE_COUNT; an empty result
   * tells playNextCore to stop as usual.
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
