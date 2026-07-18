/**
 * Typed client for the PepeAudio dashboard API. Mirrors the server DTOs in
 * src/web/bridge/types.ts. All state-changing requests carry the CSRF header the
 * server requires; a 401 anywhere means the session expired → the app returns to
 * the login screen.
 */

export type SourceType = 'youtube' | 'spotify' | 'soundcloud' | 'applemusic';
export type LoopMode = 'off' | 'track' | 'queue';
export type AuraToggle = 'off' | 'on';
export type PermissionMode = 'same-voice-channel' | 'dj-role' | 'requester-only';
export type PlayerStatus = 'idle' | 'playing' | 'paused';

export interface QueueItemDTO {
  id: string;
  title: string;
  artist: string;
  durationMs: number | null;
  thumbnailUrl: string | null;
  sourceType: SourceType;
  sourceUrl: string;
  requestedBy: string;
  requesterName: string | null;
  requesterAvatarUrl: string | null;
}

export interface ViewerCapabilities {
  canControl: boolean;
  denyReason: string | null;
  inBotVoiceChannel: boolean;
}

export interface GuildSnapshot {
  guildId: string;
  status: PlayerStatus;
  current: QueueItemDTO | null;
  elapsedMs: number;
  queue: QueueItemDTO[];
  history: QueueItemDTO[];
  loopMode: LoopMode;
  shuffleEnabled: boolean;
  autoplay: boolean;
  volume: number;
  hrirMode: AuraToggle;
  aura360Mode: AuraToggle;
  hrirProfile: string | null;
  auraPresets: Array<{ id: string; label: string }>;
  stay247: boolean;
  permissionMode: PermissionMode;
  voiceChannelId: string;
  lastError: string | null;
  auraEnabled: boolean;
  viewer: ViewerCapabilities;
}

export interface SearchCandidate {
  title: string;
  author: string;
  url: string;
  thumbnailUrl: string;
}

export interface GuildSummary {
  guildId: string;
  name: string;
  iconUrl: string | null;
  hasActiveSession: boolean;
  status: PlayerStatus;
  currentTitle: string | null;
}

export interface Me {
  userId: string;
  username: string;
  avatarUrl: string | null;
}

export type WebCommand =
  | { type: 'skip' }
  | { type: 'previous' }
  | { type: 'pause' }
  | { type: 'resume' }
  | { type: 'togglePlayPause' }
  | { type: 'stop' }
  | { type: 'toggleShuffle' }
  | { type: 'setVolume'; percent: number }
  | { type: 'setLoopMode'; mode: LoopMode }
  | { type: 'setAutoplay'; enabled: boolean }
  | { type: 'setStay247'; enabled: boolean }
  | { type: 'setHrir'; mode: AuraToggle }
  | { type: 'setAura360'; mode: AuraToggle }
  | { type: 'setAuraPreset'; id: string }
  | { type: 'removeQueueItem'; id: string }
  | { type: 'moveQueueItem'; id: string; toIndex: number }
  | { type: 'jumpTo'; id: string }
  | { type: 'seek'; positionMs: number }
  | { type: 'clearQueue' }
  | { type: 'addTrack'; query: string }
  | { type: 'loadPlaylist'; sourceUrls: string[] };

export interface CommandResult {
  ok: boolean;
  error?: string;
  snapshot?: GuildSnapshot | null;
}

export interface PlaylistTrackDTO {
  sourceUrl: string;
  title: string;
  artist: string;
  thumbnailUrl: string | null;
  sourceType: SourceType;
  durationMs: number | null;
}

export interface PlaylistSummary {
  id: string;
  name: string;
  trackCount: number;
  updatedAt: number;
}

export interface PlaylistDetail extends PlaylistSummary {
  tracks: PlaylistTrackDTO[];
}

const CSRF_HEADER = 'X-Requested-With';
const CSRF_VALUE = 'pepe-dashboard';

/** Thrown when the session is gone; the app catches it and shows the login screen. */
export class UnauthorizedError extends Error {
  constructor() {
    super('unauthorized');
  }
}

async function request<T>(method: string, path: string, body?: unknown): Promise<T> {
  const headers: Record<string, string> = {};
  const isWrite = method !== 'GET' && method !== 'HEAD';
  if (isWrite) {
    headers[CSRF_HEADER] = CSRF_VALUE;
    if (body !== undefined) headers['Content-Type'] = 'application/json';
  }
  const res = await fetch(path, {
    method,
    headers,
    credentials: 'same-origin',
    body: body !== undefined ? JSON.stringify(body) : undefined,
  });
  if (res.status === 401) throw new UnauthorizedError();
  const text = await res.text();
  const data = text ? (JSON.parse(text) as T) : ({} as T);
  if (!res.ok) {
    const message = (data as { error?: string }).error ?? `HTTP ${res.status}`;
    throw new Error(message);
  }
  return data;
}

export const api = {
  async getMe(): Promise<Me | null> {
    try {
      return await request<Me>('GET', '/api/me');
    } catch (err) {
      if (err instanceof UnauthorizedError) return null;
      throw err;
    }
  },

  getGuilds(): Promise<{ guilds: GuildSummary[] }> {
    return request('GET', '/api/guilds');
  },

  getSnapshot(guildId: string): Promise<{ snapshot: GuildSnapshot | null }> {
    return request('GET', `/api/guilds/${guildId}`);
  },

  sendCommand(guildId: string, command: WebCommand): Promise<CommandResult> {
    return request('POST', `/api/guilds/${guildId}/command`, { command });
  },

  search(query: string): Promise<{ candidates: SearchCandidate[] }> {
    return request('POST', '/api/search', { query });
  },

  logout(): Promise<void> {
    return request('POST', '/auth/logout');
  },

  listPlaylists(): Promise<{ playlists: PlaylistSummary[] }> {
    return request('GET', '/api/playlists');
  },
  getPlaylist(id: string): Promise<{ playlist: PlaylistDetail }> {
    return request('GET', `/api/playlists/${id}`);
  },
  createPlaylist(name: string): Promise<{ playlist: PlaylistSummary }> {
    return request('POST', '/api/playlists', { name });
  },
  renamePlaylist(id: string, name: string): Promise<{ playlist: PlaylistDetail }> {
    return request('PATCH', `/api/playlists/${id}`, { name });
  },
  replacePlaylistTracks(id: string, tracks: PlaylistTrackDTO[]): Promise<{ playlist: PlaylistDetail }> {
    return request('PATCH', `/api/playlists/${id}`, { tracks });
  },
  deletePlaylist(id: string): Promise<{ ok: boolean }> {
    return request('DELETE', `/api/playlists/${id}`);
  },
  addPlaylistTrack(id: string, track: PlaylistTrackDTO): Promise<{ playlist: PlaylistDetail }> {
    return request('POST', `/api/playlists/${id}/tracks`, { track });
  },
  /** Imports a provider playlist/album URL into a saved playlist server-side. */
  importPlaylist(id: string, url: string): Promise<{ playlist: PlaylistDetail; added: number }> {
    return request('POST', `/api/playlists/${id}/import`, { url });
  },
};

/**
 * Subscribes to a guild's live state via SSE. `onSnapshot` fires with each
 * snapshot (or null when the session ends). `onStatus` reports connectivity
 * (EventSource auto-reconnects; it flips to false on drop, true on (re)open).
 * Returns a close function.
 */
export function subscribeToGuild(
  guildId: string,
  onSnapshot: (snapshot: GuildSnapshot | null) => void,
  onStatus?: (connected: boolean) => void,
): () => void {
  const source = new EventSource(`/api/guilds/${guildId}/events`, { withCredentials: true });
  source.onopen = () => onStatus?.(true);
  source.onmessage = (event) => {
    onStatus?.(true);
    try {
      onSnapshot(JSON.parse(event.data) as GuildSnapshot | null);
    } catch {
      /* ignore malformed frame */
    }
  };
  source.onerror = () => onStatus?.(false);
  return () => source.close();
}
