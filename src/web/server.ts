/**
 * The node:http server + request dispatch. Applies the base security headers to
 * every response, routes API/auth requests via the Router, and falls back to
 * static file serving (with SPA fallback) for unmatched GETs.
 */
import http from 'node:http';
import { logger } from '../logger.js';
import { applyBaseHeaders, json, text } from './http/respond.js';
import type { Router } from './http/router.js';
import { serveStatic } from './routes/static.js';

export interface HttpServerHandle {
  close(): Promise<void>;
}

export interface StartHttpServerOptions {
  router: Router;
  clientDir: string;
  bindHost: string;
  port: number;
}

export function startHttpServer(opts: StartHttpServerOptions): HttpServerHandle {
  const server = http.createServer((req, res) => {
    void handleRequest(opts, req, res);
  });

  server.listen(opts.port, opts.bindHost, () => {
    logger.info({ host: opts.bindHost, port: opts.port }, 'Web dashboard listening');
  });

  return {
    close(): Promise<void> {
      return new Promise((resolve) => {
        server.close(() => resolve());
        // Don't let lingering keep-alive/SSE sockets block shutdown.
        server.closeAllConnections?.();
      });
    },
  };
}

async function handleRequest(opts: StartHttpServerOptions, req: http.IncomingMessage, res: http.ServerResponse): Promise<void> {
  applyBaseHeaders(res);

  let url: URL;
  try {
    url = new URL(req.url ?? '/', 'http://localhost');
  } catch {
    text(res, 400, 'Bad Request');
    return;
  }

  const method = (req.method ?? 'GET').toUpperCase();

  try {
    const matched = opts.router.match(method, url.pathname);
    if (matched) {
      await matched.handler({ req, res, url, method, params: matched.params });
      return;
    }

    // Unmatched /api and /auth routes must NOT fall through to the SPA — return a
    // proper 404/405 so the client sees a real error, not index.html.
    if (url.pathname.startsWith('/api/') || url.pathname.startsWith('/auth/')) {
      json(res, 404, { error: 'Not found' });
      return;
    }

    if (method === 'GET' || method === 'HEAD') {
      await serveStatic(res, opts.clientDir, url.pathname);
      return;
    }

    text(res, 405, 'Method Not Allowed');
  } catch (err) {
    logger.error({ err, path: url.pathname, method }, 'Web request handler threw');
    if (!res.headersSent) json(res, 500, { error: 'Internal server error' });
    else res.destroy();
  }
}
