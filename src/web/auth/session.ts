/**
 * In-memory web session store. Sessions live only in the process that runs the
 * web server (the ShardingManager in sharded mode, or the single Bot process),
 * not in the shared pepeaudio.sqlite — that keeps the web server from adding
 * another writer to a file every shard already writes. A process restart drops
 * sessions and the user re-authenticates, which is an acceptable trade for not
 * touching the shared DB. Discord access/refresh tokens are not stored.
 */
import { randomBytes } from 'node:crypto';

export interface WebSession {
  id: string;
  userId: string;
  username: string;
  avatar: string | null;
  /** Guild ids the user belongs to (from the OAuth `guilds` scope), refreshed on a TTL. */
  guildIds: string[];
  guildsFetchedAt: number;
  createdAt: number;
  lastSeenAt: number;
}

const SESSION_TTL_MS = 7 * 24 * 60 * 60 * 1000; // 7 days of inactivity
const SWEEP_INTERVAL_MS = 60 * 60 * 1000; // hourly

export class SessionStore {
  private readonly sessions = new Map<string, WebSession>();
  private readonly sweepTimer: NodeJS.Timeout;

  constructor() {
    this.sweepTimer = setInterval(() => this.sweep(), SWEEP_INTERVAL_MS);
    this.sweepTimer.unref();
  }

  create(params: { userId: string; username: string; avatar: string | null; guildIds: string[] }): WebSession {
    const now = Date.now();
    const session: WebSession = {
      id: randomBytes(32).toString('base64url'),
      userId: params.userId,
      username: params.username,
      avatar: params.avatar,
      guildIds: params.guildIds,
      guildsFetchedAt: now,
      createdAt: now,
      lastSeenAt: now,
    };
    this.sessions.set(session.id, session);
    return session;
  }

  /** Returns the session and updates lastSeenAt, or null if unknown/expired. */
  get(id: string | undefined): WebSession | null {
    if (!id) return null;
    const session = this.sessions.get(id);
    if (!session) return null;
    const now = Date.now();
    if (now - session.lastSeenAt > SESSION_TTL_MS) {
      this.sessions.delete(id);
      return null;
    }
    session.lastSeenAt = now;
    return session;
  }

  updateGuilds(id: string, guildIds: string[]): void {
    const session = this.sessions.get(id);
    if (!session) return;
    session.guildIds = guildIds;
    session.guildsFetchedAt = Date.now();
  }

  destroy(id: string | undefined): void {
    if (id) this.sessions.delete(id);
  }

  private sweep(): void {
    const cutoff = Date.now() - SESSION_TTL_MS;
    for (const [id, session] of this.sessions) {
      if (session.lastSeenAt < cutoff) this.sessions.delete(id);
    }
  }

  close(): void {
    clearInterval(this.sweepTimer);
    this.sessions.clear();
  }
}
