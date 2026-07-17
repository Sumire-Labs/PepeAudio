/**
 * Serves the built React frontend (dist/web-client). Path-traversal-guarded and
 * with an SPA fallback: any GET that isn't an existing file and isn't an /api or
 * /auth route returns index.html so client-side routing works. No dependencies.
 */
import { createReadStream } from 'node:fs';
import { stat } from 'node:fs/promises';
import path from 'node:path';
import type { ServerResponse } from 'node:http';
import { text } from '../http/respond.js';

const MIME_TYPES: Record<string, string> = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.mjs': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.jpeg': 'image/jpeg',
  '.gif': 'image/gif',
  '.webp': 'image/webp',
  '.ico': 'image/x-icon',
  '.woff': 'font/woff',
  '.woff2': 'font/woff2',
  '.txt': 'text/plain; charset=utf-8',
  '.map': 'application/json; charset=utf-8',
};

async function tryStatFile(filePath: string): Promise<boolean> {
  try {
    const s = await stat(filePath);
    return s.isFile();
  } catch {
    return false;
  }
}

function sendFile(res: ServerResponse, filePath: string, cacheable: boolean): void {
  const ext = path.extname(filePath).toLowerCase();
  const contentType = MIME_TYPES[ext] ?? 'application/octet-stream';
  res.setHeader('Content-Type', contentType);
  // Vite emits content-hashed asset filenames, so hashed assets can be cached
  // hard; index.html must never be cached (it references the current hashes).
  res.setHeader('Cache-Control', cacheable ? 'public, max-age=31536000, immutable' : 'no-cache');
  const stream = createReadStream(filePath);
  stream.on('error', () => {
    if (!res.headersSent) text(res, 500, 'Internal Server Error');
    else res.destroy();
  });
  stream.pipe(res);
}

/**
 * Resolves and serves a static asset for a GET request whose pathname is
 * `urlPath`. Returns true if it handled the response, false if the caller should
 * treat it as not-found (it always handles GETs via the SPA fallback, so this
 * effectively always returns true for GETs under a valid clientDir).
 */
export async function serveStatic(res: ServerResponse, clientDir: string, urlPath: string): Promise<void> {
  const rootDir = path.resolve(clientDir);
  // Normalize and confine to rootDir (defeat ../ traversal).
  const decoded = decodeURIComponent(urlPath.split('?')[0] ?? '/');
  const relative = decoded === '/' ? 'index.html' : decoded.replace(/^\/+/, '');
  const candidate = path.resolve(rootDir, relative);

  if (candidate !== rootDir && !candidate.startsWith(rootDir + path.sep)) {
    text(res, 403, 'Forbidden');
    return;
  }

  if (await tryStatFile(candidate)) {
    // Hash assets (anything under an assets/ dir or with a hash-looking name)
    // are safe to cache immutably; treat index.html and root as non-cacheable.
    const isHtml = path.extname(candidate).toLowerCase() === '.html';
    sendFile(res, candidate, !isHtml);
    return;
  }

  // SPA fallback → index.html.
  const indexPath = path.join(rootDir, 'index.html');
  if (await tryStatFile(indexPath)) {
    sendFile(res, indexPath, false);
    return;
  }

  text(res, 404, 'Dashboard frontend not built. Run the web-client build (see docs).');
}
