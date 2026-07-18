/**
 * Small response helpers over node:http — JSON/text/redirect, a strict security
 * header set applied to every response, and a size-capped body reader.
 */
import type { IncomingMessage, ServerResponse } from 'node:http';

/** Max bytes accepted for a request body (command payloads are tiny). */
export const MAX_BODY_BYTES = 16 * 1024;

/**
 * Content Security Policy for the dashboard. Scripts are same-origin only (Vite
 * emits hashed files — no inline JS). `style-src 'unsafe-inline'` is needed for
 * dynamic inline styles (progress-bar width, ambient artwork opacity); it does
 * not weaken script protection. Artwork/avatars load from external CDNs, hence
 * `img-src https:`. Everything else is locked to 'self'. `frame-ancestors 'none'`
 * plus X-Frame-Options: DENY block clickjacking.
 */
const CSP = [
  "default-src 'self'",
  "script-src 'self'",
  "style-src 'self' 'unsafe-inline'",
  "img-src 'self' https: data:",
  "connect-src 'self'",
  "font-src 'self'",
  "media-src 'none'",
  "object-src 'none'",
  "base-uri 'self'",
  "form-action 'self'",
  "frame-ancestors 'none'",
].join('; ');

/** Applied to EVERY response (static, api, sse, oauth). */
export function applyBaseHeaders(res: ServerResponse): void {
  res.setHeader('X-Content-Type-Options', 'nosniff');
  res.setHeader('X-Frame-Options', 'DENY');
  res.setHeader('Referrer-Policy', 'no-referrer');
  res.setHeader('Content-Security-Policy', CSP);
  // The dashboard is same-origin only — no CORS headers are ever set, so
  // cross-origin fetches (and thus cross-origin custom headers) are impossible.
}

export function json(res: ServerResponse, status: number, body: unknown): void {
  const payload = JSON.stringify(body);
  res.writeHead(status, { 'Content-Type': 'application/json; charset=utf-8' });
  res.end(payload);
}

export function text(res: ServerResponse, status: number, body: string, contentType = 'text/plain; charset=utf-8'): void {
  res.writeHead(status, { 'Content-Type': contentType });
  res.end(body);
}

export function redirect(res: ServerResponse, location: string, status = 302, setCookies?: string[]): void {
  const headers: Record<string, string | string[]> = { Location: location };
  if (setCookies && setCookies.length) headers['Set-Cookie'] = setCookies;
  res.writeHead(status, headers);
  res.end();
}

export function noContent(res: ServerResponse, setCookies?: string[]): void {
  if (setCookies && setCookies.length) res.setHeader('Set-Cookie', setCookies);
  res.writeHead(204);
  res.end();
}

/**
 * Reads the request body up to MAX_BODY_BYTES, destroying the socket if the
 * client tries to send more (a cheap DoS guard). Resolves to a Buffer.
 */
export function readBody(req: IncomingMessage, maxBytes = MAX_BODY_BYTES): Promise<Buffer> {
  return new Promise((resolve, reject) => {
    const chunks: Buffer[] = [];
    let total = 0;
    req.on('data', (chunk: Buffer) => {
      total += chunk.length;
      if (total > maxBytes) {
        reject(new Error('Request body too large'));
        req.destroy();
        return;
      }
      chunks.push(chunk);
    });
    req.on('end', () => resolve(Buffer.concat(chunks)));
    req.on('error', reject);
  });
}

/** Reads + parses a JSON body. Returns null on parse failure or oversize (caller sends 400). */
export async function readJson<T = unknown>(req: IncomingMessage, maxBytes = MAX_BODY_BYTES): Promise<T | null> {
  try {
    const buf = await readBody(req, maxBytes);
    if (buf.length === 0) return null;
    return JSON.parse(buf.toString('utf8')) as T;
  } catch {
    return null;
  }
}
