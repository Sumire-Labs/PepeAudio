/**
 * User-scoped saved-playlist CRUD. Loading a playlist into a guild's queue is
 * NOT here — the frontend fetches the detail then sends a `loadPlaylist` command
 * through /api/guilds/:id/command, so the same-VC authorization applies. These
 * routes only touch the per-user web DB (no bridge, no bot).
 */
import type { Router } from '../http/router.js';
import type { RequestContext } from '../http/router.js';
import type { WebServices } from '../services.js';
import { json, readJson } from '../http/respond.js';
import { getSession, passesCsrf } from '../auth/guard.js';
import type { WebSession } from '../auth/session.js';

export function registerPlaylistRoutes(router: Router, services: WebServices): void {
  const { env, sessions, playlists } = services;

  const auth = (ctx: RequestContext): WebSession | null => {
    const session = getSession(ctx, sessions, env.sessionSecret);
    if (!session) json(ctx.res, 401, { error: 'unauthenticated' });
    return session;
  };

  /** Session + CSRF for state-changing routes. */
  const authWrite = (ctx: RequestContext): WebSession | null => {
    const session = auth(ctx);
    if (!session) return null;
    if (!passesCsrf(ctx, env.publicOrigin)) {
      json(ctx.res, 403, { error: 'csrf' });
      return null;
    }
    return session;
  };

  router.add('GET', '/api/playlists', (ctx) => {
    const session = auth(ctx);
    if (!session) return;
    json(ctx.res, 200, { playlists: playlists.list(session.userId) });
  });

  router.add('POST', '/api/playlists', async (ctx) => {
    const session = authWrite(ctx);
    if (!session) return;
    const body = await readJson<{ name?: unknown }>(ctx.req);
    const result = playlists.create(session.userId, body?.name);
    if ('error' in result) {
      json(ctx.res, 400, result);
      return;
    }
    json(ctx.res, 201, { playlist: result });
  });

  router.add('GET', '/api/playlists/:id', (ctx) => {
    const session = auth(ctx);
    if (!session) return;
    const detail = playlists.get(session.userId, ctx.params.id!);
    if (!detail) {
      json(ctx.res, 404, { error: 'not_found' });
      return;
    }
    json(ctx.res, 200, { playlist: detail });
  });

  router.add('PATCH', '/api/playlists/:id', async (ctx) => {
    const session = authWrite(ctx);
    if (!session) return;
    const body = await readJson<{ name?: unknown; tracks?: unknown }>(ctx.req);
    if (!body) {
      json(ctx.res, 400, { error: 'bad_request' });
      return;
    }
    if (body.name !== undefined) {
      if (!playlists.rename(session.userId, ctx.params.id!, body.name)) {
        json(ctx.res, 400, { error: '名前を変更できませんでした。' });
        return;
      }
    }
    if (body.tracks !== undefined) {
      const result = playlists.replaceTracks(session.userId, ctx.params.id!, body.tracks);
      if ('error' in result) {
        json(ctx.res, 400, result);
        return;
      }
    }
    const detail = playlists.get(session.userId, ctx.params.id!);
    json(ctx.res, 200, { playlist: detail });
  });

  router.add('DELETE', '/api/playlists/:id', (ctx) => {
    const session = authWrite(ctx);
    if (!session) return;
    if (!playlists.delete(session.userId, ctx.params.id!)) {
      json(ctx.res, 404, { error: 'not_found' });
      return;
    }
    json(ctx.res, 200, { ok: true });
  });

  router.add('POST', '/api/playlists/:id/tracks', async (ctx) => {
    const session = authWrite(ctx);
    if (!session) return;
    const body = await readJson<{ track?: unknown }>(ctx.req);
    const result = playlists.addTrack(session.userId, ctx.params.id!, body?.track);
    if ('error' in result) {
      json(ctx.res, 400, result);
      return;
    }
    const detail = playlists.get(session.userId, ctx.params.id!);
    json(ctx.res, 200, { playlist: detail });
  });
}
