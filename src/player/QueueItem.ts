import { randomUUID } from 'node:crypto';
import type { Readable } from 'node:stream';
import type { StreamType } from '@discordjs/voice';

export type SourceType = 'youtube' | 'spotify' | 'soundcloud' | 'applemusic';

export interface QueueItem {
  id: string;
  title: string;
  artist: string;
  /** null for live streams / unknown-duration content. */
  durationMs: number | null;
  thumbnailUrl: string | null;
  sourceType: SourceType;
  sourceUrl: string;
  requestedBy: string;
  /**
   * Call again for every (re)play: a consumed stream can't be rewound, and
   * signed source URLs can expire while queued.
   */
  getStream: () => Promise<{ stream: Readable; inputType?: StreamType }>;
  /**
   * Seek offset from a timestamped link (e.g. YouTube `?t=`). GuildPlayer
   * consumes it exactly once — applied on the FIRST playback then cleared — so
   * a later loop/repeat or /previous plays the full track, not the timestamp.
   */
  initialOffsetMs: number | null;
  /**
   * Optional warm-the-cache hook: does getStream()'s work minus opening the
   * stream, memoized. Fire-and-forget on the upcoming queue[0]; never required
   * for correctness.
   */
  prefetch?: () => Promise<void>;
}

export function createQueueItem(params: Omit<QueueItem, 'id' | 'initialOffsetMs'> & { initialOffsetMs?: number | null }): QueueItem {
  return { id: randomUUID(), initialOffsetMs: null, ...params };
}
