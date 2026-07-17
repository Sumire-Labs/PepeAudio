/**
 * HMAC-signed cookie values and cookie header (de)serialization, using only
 * node:crypto. A signed value is `<value>.<base64url(HMAC-SHA256(secret, value))>`;
 * verify() recomputes the MAC and compares in constant time. Used for both the
 * session id cookie and the OAuth `state` cookie.
 */
import { createHmac, timingSafeEqual } from 'node:crypto';

function mac(value: string, secret: string): string {
  return createHmac('sha256', secret).update(value).digest('base64url');
}

/** Returns `<value>.<sig>`. `value` must not contain a '.' collision issue — it may (we split on the LAST dot). */
export function sign(value: string, secret: string): string {
  return `${value}.${mac(value, secret)}`;
}

/** Returns the original value if the signature is valid, else null. Constant-time compare. */
export function verify(signed: string, secret: string): string | null {
  const lastDot = signed.lastIndexOf('.');
  if (lastDot <= 0) return null;
  const value = signed.slice(0, lastDot);
  const providedSig = signed.slice(lastDot + 1);
  const expectedSig = mac(value, secret);
  const a = Buffer.from(providedSig);
  const b = Buffer.from(expectedSig);
  if (a.length !== b.length) return null;
  return timingSafeEqual(a, b) ? value : null;
}

/** Parses a `Cookie:` header into a name→value map (values URI-decoded). */
export function parseCookies(header: string | undefined): Record<string, string> {
  const out: Record<string, string> = {};
  if (!header) return out;
  for (const part of header.split(';')) {
    const eq = part.indexOf('=');
    if (eq === -1) continue;
    const name = part.slice(0, eq).trim();
    const value = part.slice(eq + 1).trim();
    if (!name) continue;
    try {
      out[name] = decodeURIComponent(value);
    } catch {
      out[name] = value;
    }
  }
  return out;
}

export interface CookieOptions {
  maxAgeSeconds?: number;
  httpOnly?: boolean;
  secure?: boolean;
  sameSite?: 'Strict' | 'Lax' | 'None';
  path?: string;
  /** Set to expire the cookie immediately (delete). */
  expire?: boolean;
}

/** Serializes a `Set-Cookie` value. */
export function serializeCookie(name: string, value: string, opts: CookieOptions = {}): string {
  const parts = [`${name}=${encodeURIComponent(value)}`];
  parts.push(`Path=${opts.path ?? '/'}`);
  parts.push(`SameSite=${opts.sameSite ?? 'Lax'}`);
  if (opts.httpOnly !== false) parts.push('HttpOnly');
  if (opts.secure) parts.push('Secure');
  if (opts.expire) {
    parts.push('Max-Age=0');
    parts.push('Expires=Thu, 01 Jan 1970 00:00:00 GMT');
  } else if (opts.maxAgeSeconds !== undefined) {
    parts.push(`Max-Age=${Math.floor(opts.maxAgeSeconds)}`);
  }
  return parts.join('; ');
}
