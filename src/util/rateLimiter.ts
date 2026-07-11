const lastActionAt = new Map<string, number>();

// Longer than any cooldown actually used in this bot (max is PLAY_COOLDOWN_MS,
// a few seconds) - this is purely to bound memory over a long-running process,
// not a functional cooldown window.
const ENTRY_MAX_AGE_MS = 60 * 60 * 1000;
const SWEEP_INTERVAL_MS = 10 * 60 * 1000;

/** Returns true (and records "now") if `key` hasn't acted within `cooldownMs`; false if still on cooldown. */
export function checkCooldown(scope: string, key: string, cooldownMs: number): boolean {
  const mapKey = `${scope}:${key}`;
  const now = Date.now();
  const last = lastActionAt.get(mapKey) ?? 0;
  if (now - last < cooldownMs) return false;
  lastActionAt.set(mapKey, now);
  return true;
}

/**
 * `lastActionAt` previously grew forever - one entry per (scope, userId) pair
 * ever seen, never removed, for the life of the process. Periodically sweep
 * out anything old enough that it can no longer affect any real cooldown check.
 */
const sweepTimer = setInterval(() => {
  const cutoff = Date.now() - ENTRY_MAX_AGE_MS;
  for (const [key, timestamp] of lastActionAt) {
    if (timestamp < cutoff) lastActionAt.delete(key);
  }
}, SWEEP_INTERVAL_MS);
sweepTimer.unref();
