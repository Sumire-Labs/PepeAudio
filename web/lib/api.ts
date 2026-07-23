// SPDX-License-Identifier: Apache-2.0
import type {
  AdminOverview, Guild, Me, PlayerSnapshot, PlaylistDetail, PlaylistSummary,
  PlaylistTrack, SearchCandidate,
} from "./types";

// Same-origin fetch (cookies flow via Next rewrites to the backend).
export class UnauthorizedError extends Error {
  constructor() {
    super("unauthorized");
  }
}

async function request<T>(method: string, path: string, body?: unknown): Promise<T> {
  const res = await fetch(path, {
    method,
    credentials: "include",
    headers: body !== undefined ? { "Content-Type": "application/json" } : undefined,
    body: body !== undefined ? JSON.stringify(body) : undefined,
  });
  if (res.status === 401) throw new UnauthorizedError();
  const text = await res.text();
  const data = text ? (JSON.parse(text) as T) : ({} as T);
  if (!res.ok) throw new Error((data as { error?: string }).error ?? `HTTP ${res.status}`);
  return data;
}

export const api = {
  loginUrl: "/api/auth/login",
  me: () => request<Me>("GET", "/api/auth/me"),
  guilds: () => request<Guild[]>("GET", "/api/guilds"),
  player: (guildId: string) => request<PlayerSnapshot>("GET", `/api/guilds/${guildId}/player`),
  adminOverview: () => request<AdminOverview>("GET", "/api/admin/overview"),
  logout: () => request<void>("POST", "/api/auth/logout"),

  search: (query: string) => request<{ candidates: SearchCandidate[] }>("POST", "/api/search", { query }),

  // Playlists
  listPlaylists: () => request<{ playlists: PlaylistSummary[] }>("GET", "/api/playlists"),
  getPlaylist: (id: string) => request<{ playlist: PlaylistDetail }>("GET", `/api/playlists/${id}`),
  createPlaylist: (name: string) => request<{ playlist: PlaylistSummary }>("POST", "/api/playlists", { name }),
  renamePlaylist: (id: string, name: string) =>
    request<{ playlist: PlaylistDetail }>("PATCH", `/api/playlists/${id}`, { name }),
  replacePlaylistTracks: (id: string, tracks: PlaylistTrack[]) =>
    request<{ playlist: PlaylistDetail }>("PATCH", `/api/playlists/${id}`, { tracks }),
  deletePlaylist: (id: string) => request<{ ok: boolean }>("DELETE", `/api/playlists/${id}`),
  addPlaylistTrack: (id: string, track: PlaylistTrack) =>
    request<{ playlist: PlaylistDetail }>("POST", `/api/playlists/${id}/tracks`, { track }),
  importPlaylist: (id: string, url: string) =>
    request<{ playlist: PlaylistDetail; added: number }>("POST", `/api/playlists/${id}/import`, { url }),
};
