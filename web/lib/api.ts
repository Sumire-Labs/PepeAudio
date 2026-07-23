// SPDX-License-Identifier: Apache-2.0
import type { AdminOverview, Guild, Me, PlayerState } from "./types";

// Same-origin fetch (cookies flow via Next rewrites to the backend).
async function get<T>(path: string): Promise<T> {
  const res = await fetch(path, { credentials: "include" });
  if (res.status === 401) throw new Error("unauthorized");
  if (!res.ok) throw new Error(`request failed: ${res.status}`);
  return res.json() as Promise<T>;
}

export const api = {
  me: () => get<Me>("/api/auth/me"),
  guilds: () => get<Guild[]>("/api/guilds"),
  player: (guildId: string) => get<PlayerState>(`/api/guilds/${guildId}/player`),
  adminOverview: () => get<AdminOverview>("/api/admin/overview"),
  loginUrl: "/api/auth/login",
  logout: () => fetch("/api/auth/logout", { method: "POST", credentials: "include" }),
};
