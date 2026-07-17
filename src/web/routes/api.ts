/**
 * JSON API: identity, guild list, per-guild snapshot, and the command endpoint.
 * Every route requires a valid session; the command endpoint additionally
 * requires the CSRF header + same-origin, membership in the guild, and a blunt
 * per-user rate limit on top of the per-action cooldowns inside the executor.
 */
import type { Router } from '../http/router.js';
import type { RequestContext } from '../http/router.js';
import type { WebServices } from '../services.js';
import { checkCooldown } from '../../util/rateLimiter.js';
import { json, readJson } from '../http/respond.js';
import { getSession, passesCsrf } from '../auth/guard.js';
import type { WebSession } from '../auth/session.js';
import type { WebCommand } from '../bridge/types.js';

const VALID_COMMAND_TYPES = new Set<WebCommand['type']>([
  'skip', 'previous', 'pause', 'resume', 'togglePlayPause', 'stop', 'toggleShuffle',
  'setVolume', 'setLoopMode', 'setAutoplay', 'setStay247', 'setHrir', 'setAura360',
  'setAuraPreset', 'removeQueueItem', 'moveQueueItem', 'jumpTo', 'seek', 'clearQueue',
  'addTrack', 'loadPlaylist',
]);

function avatarUrl(userId: string, avatar: string | null): string | null {
  if (!avatar) return null;
  return `https://cdn.discordapp.com/avatars/${userId}/${avatar}.png?size=128`;
}

function parseCommand(body: unknown): WebCommand | null {
  if (!body || typeof body !== 'object') return null;
  const command = (body as { command?: unknown }).command;
  if (!command || typeof command !== 'object') return null;
  const type = (command as { type?: unknown }).type;
  if (typeof type !== 'string' || !VALID_COMMAND_TYPES.has(type as WebCommand['type'])) return null;
  return command as WebCommand;
}

export function registerApiRoutes(router: Router, services: WebServices): void {
  const { env, sessions, bridge } = services;

  const requireSession = (ctx: RequestContext): WebSession | null => {
    const session = getSession(ctx, sessions, env.sessionSecret);
    if (!session) json(ctx.res, 401, { error: 'unauthenticated' });
    return session;
  };

  router.add('GET', '/api/me', (ctx) => {
    const session = requireSession(ctx);
    if (!session) return;
    json(ctx.res, 200, {
      userId: session.userId,
      username: session.username,
      avatarUrl: avatarUrl(session.userId, session.avatar),
    });
  });

  router.add('GET', '/api/guilds', async (ctx) => {
    const session = requireSession(ctx);
    if (!session) return;
    const guilds = await bridge.listControllableGuilds(session.guildIds, session.userId);
    json(ctx.res, 200, { guilds });
  });

  router.add('GET', '/api/guilds/:id', async (ctx) => {
    const session = requireSession(ctx);
    if (!session) return;
    const guildId = ctx.params.id!;
    if (!session.guildIds.includes(guildId)) {
      json(ctx.res, 403, { error: 'forbidden' });
      return;
    }
    if (!checkCooldown('web:snapshot', session.userId, 200)) {
      json(ctx.res, 429, { error: 'slow_down' });
      return;
    }
    const snapshot = await bridge.getSnapshot(guildId, session.userId);
    json(ctx.res, 200, { snapshot });
  });

  router.add('POST', '/api/guilds/:id/command', async (ctx) => {
    const session = requireSession(ctx);
    if (!session) return;
    if (!passesCsrf(ctx, env.publicOrigin)) {
      json(ctx.res, 403, { error: 'csrf' });
      return;
    }
    const guildId = ctx.params.id!;
    if (!session.guildIds.includes(guildId)) {
      json(ctx.res, 403, { error: 'forbidden' });
      return;
    }
    if (!checkCooldown('web:command', session.userId, 350)) {
      json(ctx.res, 429, { error: 'slow_down' });
      return;
    }
    const command = parseCommand(await readJson(ctx.req));
    if (!command) {
      json(ctx.res, 400, { error: 'bad_command' });
      return;
    }
    const result = await bridge.runCommand(guildId, session.userId, command);
    json(ctx.res, result.ok ? 200 : 400, result);
  });

  // Search is guild-independent (it just queries YouTube), so it's not scoped to
  // a guild id. Session + CSRF + a per-user rate limit (each call hits YouTube).
  router.add('POST', '/api/search', async (ctx) => {
    const session = requireSession(ctx);
    if (!session) return;
    if (!passesCsrf(ctx, env.publicOrigin)) {
      json(ctx.res, 403, { error: 'csrf' });
      return;
    }
    if (!checkCooldown('web:search', session.userId, 1200)) {
      json(ctx.res, 429, { error: 'slow_down' });
      return;
    }
    const body = await readJson<{ query?: unknown }>(ctx.req);
    const query = typeof body?.query === 'string' ? body.query.trim() : '';
    if (!query || query.length > 200) {
      json(ctx.res, 400, { error: 'bad_query' });
      return;
    }
    const candidates = await bridge.search(query);
    json(ctx.res, 200, { candidates });
  });
}
