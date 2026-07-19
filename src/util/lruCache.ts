interface Entry<V> {
  value: V;
  expiresAt: number;
}

/**
 * LRU+TTL cache. Recency = Map insertion order: `get` re-inserts a hit (MRU),
 * `set` evicts the oldest entry over capacity. Expiry is checked lazily on `get`.
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
