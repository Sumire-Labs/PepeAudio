// Only import when WEB_DASHBOARD_ENABLED is true — this pulls in the web DB and its deps, keeping them off the default path.
import { startHttpServer } from './server.js';
import { Router } from './http/router.js';
import { SessionStore } from './auth/session.js';
import { PlaylistRepo } from './data/playlistRepo.js';
import { registerOAuthRoutes } from './routes/oauth.js';
import { registerApiRoutes } from './routes/api.js';
import { registerSseRoutes } from './routes/sse.js';
import { registerPlaylistRoutes } from './routes/playlists.js';
import type { BotBridge } from './bridge/types.js';
import type { WebEnv } from './config.js';
import type { WebServices } from './services.js';

export interface WebServerHandle {
  close(): Promise<void>;
}

export function startWebServer(opts: { bridge: BotBridge; env: WebEnv }): WebServerHandle {
  const sessions = new SessionStore();
  const playlists = new PlaylistRepo();
  const services: WebServices = { bridge: opts.bridge, sessions, env: opts.env, playlists };

  const router = new Router();
  registerOAuthRoutes(router, services);
  registerApiRoutes(router, services);
  registerSseRoutes(router, services);
  registerPlaylistRoutes(router, services);

  const http = startHttpServer({
    router,
    clientDir: opts.env.clientDir,
    bindHost: opts.env.bindHost,
    port: opts.env.port,
  });

  return {
    async close(): Promise<void> {
      await http.close();
      sessions.close();
      opts.bridge.close();
      // webDb is a process-lifetime handle; the OS reclaims it on exit.
    },
  };
}
