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

export function registerSseRoutes(router: Router, services: WebServices): void {
  const { env, sessions, bridge } = services;

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

    function cleanup(): void {
      if (closed) return;
      closed = true;
      clearInterval(heartbeat);
      unsubscribe();
    }

    ctx.req.on('close', cleanup);
    res.on('close', cleanup);
  });
}
