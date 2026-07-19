import type { RequestContext } from '../http/router.js';
import type { SessionStore, WebSession } from './session.js';
import { parseCookies, verify } from './cookies.js';

export const SESSION_COOKIE = 'pepe_sid';
export const OAUTH_STATE_COOKIE = 'pepe_oauth_state';
export const CSRF_HEADER = 'x-requested-with';
export const CSRF_HEADER_VALUE = 'pepe-dashboard';

export function getSession(ctx: RequestContext, sessions: SessionStore, secret: string): WebSession | null {
  const cookies = parseCookies(ctx.req.headers.cookie);
  const signed = cookies[SESSION_COOKIE];
  if (!signed) return null;
  const sessionId = verify(signed, secret);
  if (!sessionId) return null;
  return sessions.get(sessionId);
}

export function getSignedCookie(ctx: RequestContext, name: string, secret: string): string | null {
  const cookies = parseCookies(ctx.req.headers.cookie);
  const signed = cookies[name];
  if (!signed) return null;
  return verify(signed, secret);
}

// CSRF: require same-origin (Origin, or Referer fallback) AND the custom
// X-Requested-With header — a cross-site page can forge neither without a CORS
// grant the dashboard never issues.
export function passesCsrf(ctx: RequestContext, publicOrigin: string): boolean {
  const header = ctx.req.headers[CSRF_HEADER];
  const headerValue = Array.isArray(header) ? header[0] : header;
  if (headerValue !== CSRF_HEADER_VALUE) return false;

  const origin = ctx.req.headers.origin;
  if (typeof origin === 'string' && origin.length > 0) {
    return origin === publicOrigin;
  }
  // Some browsers omit Origin on same-origin requests; fall back to Referer.
  const referer = ctx.req.headers.referer;
  if (typeof referer === 'string' && referer.length > 0) {
    try {
      return new URL(referer).origin === publicOrigin;
    } catch {
      return false;
    }
  }
  // No Origin and no Referer → refuse (a legitimate SPA fetch sends one).
  return false;
}
