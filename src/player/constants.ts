import os from 'node:os';

export type PlaybackStatus = 'idle' | 'playing' | 'paused' | 'buffering';
export type LoopMode = 'off' | 'track' | 'queue';
export type SpatialMode = 'off' | 'on';
export type PermissionMode = 'same-voice-channel' | 'dj-role' | 'requester-only';

export const PROGRESS_BAR_WIDTH = 18;
export const PANEL_PERIODIC_REFRESH_MS = 10_000;

/**
 * Minimum interval between periodic (non-button-triggered) panel edits, as a
 * function of how many guilds are ACTIVELY playing right now. Discord's REST
 * global rate limit is roughly 50 requests/second per bot; at hundreds of
 * simultaneously-playing guilds, periodic panel edits alone could saturate
 * that and delay real interaction responses. Button-triggered edits
 * (scheduleCoalescedEdit's MIN_EDIT_GAP_MS) are unaffected — this only
 * throttles the background tick in panelManager's ensureRefreshLoop.
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

/**
 * Bounds a SINGLE resolve request. A Spotify playlist/album can hold thousands
 * of tracks, and each one costs a YouTube search (network + CPU) during
 * resolution — without a cap, one `/play <big playlist>` can pin the process on
 * thousands of sequential lookups and get the host rate-limited/banned. Full
 * albums and normal playlists fit comfortably under this.
 */
export const MAX_PLAYLIST_TRACKS = 50;

/**
 * Hard ceiling on a guild's pending queue, enforced on every enqueue. The
 * per-request cap above bounds one command; this bounds unbounded growth from
 * repeated `/play` calls (only a few seconds apart) so queue memory stays
 * bounded regardless of how the items arrive.
 */
export const MAX_QUEUE_LENGTH = 500;

export const PLAY_COOLDOWN_MS = 3_000;
export const BUTTON_COOLDOWN_MS = 1_500;
export const VOLUME_COOLDOWN_MS = 500;

export const MAX_FFMPEG_CRASH_RETRIES = 1;

/**
 * Caps concurrent sofalizer (real HRTF convolution) ffmpeg processes across all
 * guilds handled by THIS process. Under sharding (see shard.ts), each shard is
 * its own OS process with its own copy of this module-level counter, so the
 * effective cap is per-shard, not global across the whole bot - e.g. 4 shards
 * on a host with os.cpus().length reporting the same core count each cap
 * independently at that value, not divide one global budget between them.
 * Each shard process should be scheduled with that in mind (e.g. one shard per
 * host, or a per-container CPU limit that matches this assumption).
 */
export const MAX_SOFALIZER_CONCURRENCY = Math.max(1, Math.min(4, os.cpus().length - 1));

/** Same rationale as MAX_SOFALIZER_CONCURRENCY (including the per-process/per-shard caveat), for the bring-your-own HRIR (afir) path. */
export const MAX_HRIR_CONCURRENCY = Math.max(1, Math.min(4, os.cpus().length - 1));

/**
 * Capped at 100 — @discordjs/voice's inlineVolume (prism-media's VolumeTransformer)
 * is a plain multiply-then-hard-clamp with no limiter, and setVolumeLogarithmic()
 * scales exponentially (its underlying gain, not the displayed %), so e.g. the old
 * 150%/200% presets multiplied raw PCM by roughly 2.05x/3.16x — virtually guaranteed
 * to hard-clip on any already-loudness-mastered track (confirmed by reading
 * prism-media's VolumeTransformer source; this is not source-material-specific).
 * Volume here is attenuation-only, matching how most well-behaved music players
 * treat "volume" above the source's own mastered level.
 */
export const VOLUME_PRESETS = [0, 10, 25, 50, 75, 100] as const;
export const MAX_VOLUME_PERCENT = 100;

/**
 * Gates the "360° Sound" panel button/status line and the toggle handler; while
 * false a guild's persisted defaultSpatialMode is also forced to 'off' at load
 * time so no one is stuck with it silently on with no way to turn it off.
 *
 * Re-enabled after the sound-quality rework: the toggle now drives a
 * level-matched afir BRIR virtualization (audio/hrirFilterComplex.ts) — static,
 * out-of-head, full-band — with an asset-free wide fallback, replacing the raw
 * MIT KEMAR sofalizer path that sounded muffled.
 */
export const SPATIAL_AUDIO_ENABLED = true;
