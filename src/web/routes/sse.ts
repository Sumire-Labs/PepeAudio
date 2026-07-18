/**
 * Server-Sent Events stream for one guild's live player state. Sends the current
 * snapshot on connect, then a fresh snapshot on every throttled player update
 * (and `null` when the session ends). SSE (not WebSocket) keeps the dependency
 * footprint at zero and passes cleanly through reverse proxies.
 */
import type { Router } from '../http/router.js';
import type { WebServices } from '../services.js';
import { json } from '../http/respond.js';
import { getSession } from '../auth/guard.js';
import type { GuildSnapshot } from '../bridge/types.js';

const HEARTBEAT_MS = 15_000;
/** Cap concurrent SSE streams per user (multiple tabs + reconnect churn) so one account can't pin open unbounded streams. */
const MAX_SSE_PER_USER = 8;
/** Hard lifetime for one SSE stream. Bounds any connection a proxy leaves half-open (the client just reconnects). */
const MAX_STREAM_MS = 30 * 60 * 1000;

export function registerSseRoutes(router: Router, services: WebServices): void {
  const { env, sessions, bridge } = services;
  // Live SSE connection count per userId, for the concurrency cap below.
  const openByUser = new Map<string, number>();

  router.add('GET', '/api/guilds/:id/events', async (ctx) => {
    const session = getSession(ctx, sessions, env.sessionSecret);
    if (!session) {
      json(ctx.res, 401, { error: 'unauthenticated' });
      return;
    }
    const guildId = ctx.params.id!;
    if (!session.guildIds.includes(guildId)) {
      json(ctx.res, 403, { error: 'forbidden' });
      return;
    }

    const openCount = openByUser.get(session.userId) ?? 0;
    if (openCount >= MAX_SSE_PER_USER) {
      json(ctx.res, 429, { error: 'too_many_connections' });
      return;
    }
    openByUser.set(session.userId, openCount + 1);

    const res = ctx.res;
    res.writeHead(200, {
      'Content-Type': 'text/event-stream; charset=utf-8',
      'Cache-Control': 'no-cache, no-transform',
      Connection: 'keep-alive',
      // Defeat nginx proxy buffering so events flush immediately.
      'X-Accel-Buffering': 'no',
    });
    res.write('retry: 5000\n\n');

    let closed = false;
    const send = (snapshot: GuildSnapshot | null): void => {
      if (closed || res.writableEnded) return;
      try {
        res.write(`data: ${JSON.stringify(snapshot)}\n\n`);
      } catch {
        cleanup();
      }
    };

    // Initial snapshot (per-viewer capabilities accurate for the REST fetch).
    const initial = await bridge.getSnapshot(guildId, session.userId);
    send(initial);

    const unsubscribe = bridge.subscribe(guildId, session.userId, send);

    const heartbeat = setInterval(() => {
      if (closed || res.writableEnded) return;
      try {
        res.write(': ping\n\n');
      } catch {
        cleanup();
      }
    }, HEARTBEAT_MS);
    heartbeat.unref();

    // Recycle the stream after a bounded lifetime so a proxy-abandoned connection
    // can't linger forever (the browser's EventSource just reconnects).
    const lifetime = setTimeout(() => {
      try {
        res.end();
      } catch {
        /* already closing */
      }
      cleanup();
    }, MAX_STREAM_MS);
    lifetime.unref();

    function cleanup(): void {
      if (closed) return;
      closed = true;
      clearInterval(heartbeat);
      clearTimeout(lifetime);
      unsubscribe();
      const remaining = (openByUser.get(session!.userId) ?? 1) - 1;
      if (remaining <= 0) openByUser.delete(session!.userId);
      else openByUser.set(session!.userId, remaining);
    }

    ctx.req.on('close', cleanup);
    res.on('close', cleanup);
  });
}
