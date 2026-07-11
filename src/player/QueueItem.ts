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
   * Lazily resolves a fresh, playable stream. MUST be called again for every
   * (re)play attempt — a previously-consumed stream cannot be rewound, and
   * signed source URLs (YouTube/SoundCloud) can expire while queued.
   */
  getStream: () => Promise<{ stream: Readable; inputType?: StreamType }>;
  /**
   * Set when the original link included a timestamp (e.g. YouTube's `?t=`).
   * Consumed exactly once — GuildPlayer applies it as the seek offset on this
   * item's FIRST playback and then clears it, so a later loop:'track' repeat,
   * loop:'queue' wraparound, or /previous back to this same item plays the
   * full track rather than jumping back to the shared link's timestamp every time.
   */
  initialOffsetMs: number | null;
  /**
   * Optional: does whatever work getStream() would need to do EXCEPT actually
   * opening the stream (e.g. a lazily-matched item's YouTube search/scoring -
   * see youtubeMatch.ts's createLazyMatchedQueueItem), memoized so a
   * subsequent getStream() call doesn't repeat it. Called fire-and-forget for
   * the upcoming queue[0] once the current track starts (see GuildPlayer.ts) -
   * purely a warm-the-cache optimization, never required for correctness.
   */
  prefetch?: () => Promise<void>;
}

export function createQueueItem(params: Omit<QueueItem, 'id' | 'initialOffsetMs'> & { initialOffsetMs?: number | null }): QueueItem {
  return { id: randomUUID(), initialOffsetMs: null, ...params };
}
