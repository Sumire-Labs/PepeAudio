/**
 * OAuth2 login routes: /auth/login (start), /auth/callback (finish), /auth/logout.
 * The login leg is CSRF-protected by a signed `state` cookie compared in the
 * callback; the redirect_uri is a fixed env value (never taken from the request),
 * so there's no open-redirect surface.
 */
import { randomBytes } from 'node:crypto';
import type { Router } from '../http/router.js';
import type { WebServices } from '../services.js';
import { logger } from '../../logger.js';
import { checkCooldown } from '../../util/rateLimiter.js';
import { json, noContent, redirect } from '../http/respond.js';
import { serializeCookie, sign } from '../auth/cookies.js';
import { OAUTH_STATE_COOKIE, SESSION_COOKIE, getSession, getSignedCookie, passesCsrf } from '../auth/guard.js';
import { buildAuthorizeUrl, exchangeCode, fetchGuildIds, fetchUser } from '../auth/oauth.js';

const STATE_TTL_SECONDS = 600;
const SESSION_COOKIE_TTL_SECONDS = 7 * 24 * 60 * 60;

export function registerOAuthRoutes(router: Router, services: WebServices): void {
  const { env, sessions } = services;

  router.add('GET', '/auth/login', (ctx) => {
    const ip = ctx.req.socket.remoteAddress ?? 'unknown';
    if (!checkCooldown('web:login', ip, 1000)) {
      redirect(ctx.res, '/?auth=slow_down');
      return;
    }
    const state = randomBytes(16).toString('base64url');
    const stateCookie = serializeCookie(OAUTH_STATE_COOKIE, sign(state, env.sessionSecret), {
      httpOnly: true,
      secure: env.secureCookies,
      sameSite: 'Lax',
      maxAgeSeconds: STATE_TTL_SECONDS,
    });
    ctx.res.setHeader('Set-Cookie', stateCookie);
    redirect(ctx.res, buildAuthorizeUrl(env, state));
  });

  router.add('GET', '/auth/callback', async (ctx) => {
    const code = ctx.url.searchParams.get('code');
    const state = ctx.url.searchParams.get('state');
    const expectedState = getSignedCookie(ctx, OAUTH_STATE_COOKIE, env.sessionSecret);
    const clearState = serializeCookie(OAUTH_STATE_COOKIE, '', {
      httpOnly: true,
      secure: env.secureCookies,
      sameSite: 'Lax',
      expire: true,
    });

    // CSRF on the login leg: the returned state must equal our signed cookie.
    if (!code || !state || !expectedState || state !== expectedState) {
      redirect(ctx.res, '/?auth=error', 302, [clearState]);
      return;
    }

    try {
      const accessToken = await exchangeCode(env, code);
      const [user, guildIds] = await Promise.all([fetchUser(accessToken), fetchGuildIds(accessToken)]);
      const session = sessions.create({
        userId: user.id,
        username: user.global_name || user.username,
        avatar: user.avatar,
        guildIds,
      });
      const sessionCookie = serializeCookie(SESSION_COOKIE, sign(session.id, env.sessionSecret), {
        httpOnly: true,
        secure: env.secureCookies,
        sameSite: 'Lax',
        maxAgeSeconds: SESSION_COOKIE_TTL_SECONDS,
      });
      redirect(ctx.res, '/', 302, [clearState, sessionCookie]);
    } catch (err) {
      // Never leak token/exchange details to the browser.
      logger.error({ err }, 'OAuth callback failed');
      redirect(ctx.res, '/?auth=error', 302, [clearState]);
    }
  });

  router.add('POST', '/auth/logout', (ctx) => {
    if (!passesCsrf(ctx, env.publicOrigin)) {
      json(ctx.res, 403, { error: 'CSRF check failed' });
      return;
    }
    const session = getSession(ctx, sessions, env.sessionSecret);
    if (session) sessions.destroy(session.id);
    const clear = serializeCookie(SESSION_COOKIE, '', {
      httpOnly: true,
      secure: env.secureCookies,
      sameSite: 'Lax',
      expire: true,
    });
    noContent(ctx.res, [clear]);
  });
}
