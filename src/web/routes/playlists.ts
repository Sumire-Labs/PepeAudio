// Per-user web DB only. Loading into a guild's queue goes through
// /api/guilds/:id/command (same-VC authz applies there), not here.
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

  // Resolves the URL via the bot bridge (SSRF-guarded). Lazy collection tracks
  // are stored as search strings so loading resolves each one individually.
  router.add('POST', '/api/playlists/:id/import', async (ctx) => {
    const session = authWrite(ctx);
    if (!session) return;
    const detail = playlists.get(session.userId, ctx.params.id!);
    if (!detail) {
      json(ctx.res, 404, { error: 'not_found' });
      return;
    }
    const body = await readJson<{ url?: unknown }>(ctx.req);
    const url = typeof body?.url === 'string' ? body.url.trim() : '';
    if (!url || url.length > 2000) {
      json(ctx.res, 400, { error: 'URL を入力してください。' });
      return;
    }
    const resolved = await services.bridge.resolveTracks(url);
    if (resolved.error) {
      json(ctx.res, 400, { error: resolved.error });
      return;
    }
    if (resolved.tracks.length === 0) {
      json(ctx.res, 400, { error: '曲が見つかりませんでした。' });
      return;
    }
    const toAdd = resolved.tracks.map((t) => ({
      sourceUrl: t.sourceUrl,
      title: t.title,
      artist: t.artist,
      thumbnailUrl: t.thumbnailUrl,
      sourceType: t.sourceType,
      durationMs: t.durationMs,
    }));
    const result = playlists.addTracks(session.userId, ctx.params.id!, toAdd);
    if ('error' in result) {
      json(ctx.res, 400, result);
      return;
    }
    const updated = playlists.get(session.userId, ctx.params.id!);
    json(ctx.res, 200, { playlist: updated, added: result.added });
  });
}
