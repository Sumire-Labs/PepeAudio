interface Entry<V> {
  value: V;
  expiresAt: number;
}

/**
 * Minimal in-memory LRU+TTL cache with no runtime dependency (see
 * docs/performance-optimization-plan.md phase 4 — this is deliberately
 * self-implemented rather than pulling in a package). Recency is tracked via
 * Map's own insertion-order iteration: re-inserting a key on every `get` hit
 * moves it to the end (most-recently-used) position, and `set` evicts the
 * oldest (first-iterated) entry once over capacity. Expired entries are
 * discarded lazily, only when a `get` actually touches them.
 */
export class LruCache<K, V> {
  private readonly map = new Map<K, Entry<V>>();

  constructor(
    private readonly maxSize: number,
    private readonly ttlMs: number,
  ) {}

  get(key: K): V | undefined {
    const entry = this.map.get(key);
    if (!entry) return undefined;
    if (Date.now() >= entry.expiresAt) {
      this.map.delete(key);
      return undefined;
    }
    // Re-insert to mark as most-recently-used.
    this.map.delete(key);
    this.map.set(key, entry);
    return entry.value;
  }

  set(key: K, value: V): void {
    this.map.delete(key); // drop any existing entry first so a re-set also refreshes recency
    if (this.map.size >= this.maxSize) {
      const oldestKey = this.map.keys().next().value;
      if (oldestKey !== undefined) this.map.delete(oldestKey);
    }
    this.map.set(key, { value, expiresAt: Date.now() + this.ttlMs });
  }

  get size(): number {
    return this.map.size;
  }
}
