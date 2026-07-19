import os from 'node:os';

export type PlaybackStatus = 'idle' | 'playing' | 'paused' | 'buffering';
export type LoopMode = 'off' | 'track' | 'queue';
export type AuraToggle = 'off' | 'on';
export type PermissionMode = 'same-voice-channel' | 'dj-role' | 'requester-only';

export const PROGRESS_BAR_WIDTH = 18;
export const PANEL_PERIODIC_REFRESH_MS = 10_000;

/**
 * Minimum interval between periodic (non-button-triggered) panel edits, scaled
 * by active-player count: at hundreds of playing guilds, background panel edits
 * alone can saturate Discord's ~50 req/s global limit and delay interactions.
 * Button-triggered edits are unaffected.
 */
export function panelRefreshIntervalMs(activePlayerCount: number): number {
  if (activePlayerCount >= 500) return 60_000;
  if (activePlayerCount >= 300) return 30_000;
  if (activePlayerCount >= 100) return 20_000;
  return PANEL_PERIODIC_REFRESH_MS;
}

export const ALONE_TIMEOUT_MS = 60_000;
export const EMPTY_QUEUE_TIMEOUT_MS = 5 * 60_000;

export const MAX_HISTORY = 50;

/** Bounds a single resolve request: each track costs a YouTube search, so an uncapped `/play <big playlist>` can pin the process on sequential lookups and get the host rate-limited/banned. */
export const MAX_PLAYLIST_TRACKS = 50;

/** Hard ceiling on a guild's pending queue, enforced on every enqueue — bounds unbounded growth from repeated `/play` calls that each pass the per-request cap. */
export const MAX_QUEUE_LENGTH = 500;

/** Autoplay fetches this many related candidates, then de-dupes against session history down to AUTOPLAY_ENQUEUE_COUNT — the surplus leaves room to drop already-played repeats. */
export const AUTOPLAY_FETCH_LIMIT = 15;
/** Autoplay: fresh (non-repeat) related tracks enqueued each time the queue runs dry. */
export const AUTOPLAY_ENQUEUE_COUNT = 5;

export const PLAY_COOLDOWN_MS = 3_000;
export const BUTTON_COOLDOWN_MS = 1_500;
export const VOLUME_COOLDOWN_MS = 500;

export const MAX_FFMPEG_CRASH_RETRIES = 1;

/**
 * Caps concurrent sofalizer (HRTF convolution) ffmpeg processes in THIS process.
 * Under sharding each shard is a separate OS process with its own counter, so the
 * cap is per-shard, not global — schedule shards (one per host, or a per-container
 * CPU limit) with that in mind.
 */
export const MAX_SOFALIZER_CONCURRENCY = Math.max(1, Math.min(4, os.cpus().length - 1));

/** Per-process/per-shard cap (see MAX_SOFALIZER_CONCURRENCY) for the afir HRIR path. */
export const MAX_HRIR_CONCURRENCY = Math.max(1, Math.min(4, os.cpus().length - 1));

/**
 * Capped at 100: inlineVolume (prism-media's VolumeTransformer) is multiply-then-
 * hard-clamp with no limiter, so presets above 100% hard-clip loudness-mastered
 * tracks. Volume here is attenuation-only.
 */
export const VOLUME_PRESETS = [
  0, 5, 10, 15, 20, 25, 30, 35, 40, 45, 50, 55, 60, 65, 70, 75, 80, 85, 90, 95, 100,
] as const; // 5% steps, ≤25 options for Discord's select limit; DEFAULT_VOLUME_PERCENT must be one of these
export const MAX_VOLUME_PERCENT = 100;

/** Starting volume for every guild, pinned at construction (not the persisted per-guild default). Must be one of VOLUME_PRESETS. */
export const DEFAULT_VOLUME_PERCENT = 70;

/** Gates the Aura HRIR panel toggle button + handler (hrirMode persists per guild, default 'off' — opt-in effect). */
export const AURA_ENABLED = true;

/**
 * Attenuation (dB) applied to NORMAL mode. Now 0: NORMAL (Aura HRIR off) is the
 * DEFAULT and the pristine reference, so it must play at full level. This trim
 * only existed to level-match the OFF path DOWN to the quieter always-on Aura
 * HRIR back when ON was the default; with OFF as the default that would just make
 * normal playback needlessly quiet. (Opting into Aura HRIR is now ~4 dB quieter
 * than OFF — expected for an effect; recover via volume, or lower MAKEUP_HEADROOM_DB.)
 */
export const NORMAL_MODE_TRIM_DB = 0;

/** Linear gain for NORMAL_MODE_TRIM_DB — multiply a VolumeTransformer's linear volume by this to trim normal mode. */
export const NORMAL_MODE_TRIM_FACTOR = Math.pow(10, -NORMAL_MODE_TRIM_DB / 20);
