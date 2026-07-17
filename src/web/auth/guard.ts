/**
 * Session extraction and CSRF checks shared by the API/oauth routes. The session
 * id travels in an HMAC-signed HttpOnly cookie; CSRF is defended in depth by a
 * same-origin Origin/Referer check plus a custom header that cross-site requests
 * cannot set (no CORS is ever granted).
 */
import type { RequestContext } from '../http/router.js';
import type { SessionStore, WebSession } from './session.js';
import { parseCookies, verify } from './cookies.js';

export const SESSION_COOKIE = 'pepe_sid';
export const OAUTH_STATE_COOKIE = 'pepe_oauth_state';
export const CSRF_HEADER = 'x-requested-with';
export const CSRF_HEADER_VALUE = 'pepe-dashboard';

/** Returns the authenticated session, or null. Verifies the cookie signature first. */
export function getSession(ctx: RequestContext, sessions: SessionStore, secret: string): WebSession | null {
  const cookies = parseCookies(ctx.req.headers.cookie);
  const signed = cookies[SESSION_COOKIE];
  if (!signed) return null;
  const sessionId = verify(signed, secret);
  if (!sessionId) return null;
  return sessions.get(sessionId);
}

/** Reads a signed cookie's verified value, or null. */
export function getSignedCookie(ctx: RequestContext, name: string, secret: string): string | null {
  const cookies = parseCookies(ctx.req.headers.cookie);
  const signed = cookies[name];
  if (!signed) return null;
  return verify(signed, secret);
}

/**
 * CSRF check for state-changing requests: the request must be same-origin
 * (Origin, or Referer as a fallback, must match our public origin) AND carry the
 * custom X-Requested-With header. A cross-site page can forge neither without a
 * CORS grant, which the dashboard never issues.
 */
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
