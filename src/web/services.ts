/** The shared services every route module is bound to at registration time. */
import type { BotBridge } from './bridge/types.js';
import type { SessionStore } from './auth/session.js';
import type { WebEnv } from './config.js';
import type { PlaylistRepo } from './data/playlistRepo.js';

export interface WebServices {
  bridge: BotBridge;
  sessions: SessionStore;
  env: WebEnv;
  playlists: PlaylistRepo;
}
