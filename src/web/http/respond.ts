import type { IncomingMessage, ServerResponse } from 'node:http';

export const MAX_BODY_BYTES = 16 * 1024;

// style-src 'unsafe-inline' is required for dynamic inline styles (progress-bar
// width, artwork opacity) and does not weaken script protection. img-src allows
// https: for external artwork/avatar CDNs.
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

export function applyBaseHeaders(res: ServerResponse): void {
  res.setHeader('X-Content-Type-Options', 'nosniff');
  res.setHeader('X-Frame-Options', 'DENY');
  res.setHeader('Referrer-Policy', 'no-referrer');
  res.setHeader('Content-Security-Policy', CSP);
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

/** Destroys the socket if the body exceeds maxBytes (DoS guard). */
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

/** Returns null on parse failure or oversize (caller sends 400). */
export async function readJson<T = unknown>(req: IncomingMessage, maxBytes = MAX_BODY_BYTES): Promise<T | null> {
  try {
    const buf = await readBody(req, maxBytes);
    if (buf.length === 0) return null;
    return JSON.parse(buf.toString('utf8')) as T;
  } catch {
    return null;
  }
}
