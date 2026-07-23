// SPDX-License-Identifier: Apache-2.0
import type { PlaylistTrack, QueueItem } from "./types";

/** m:ss (or h:mm:ss) — null/negative/non-finite render as --:--. */
export function formatMs(ms: number | null | undefined): string {
  if (ms == null || !Number.isFinite(ms) || ms < 0) return "--:--";
  const total = Math.floor(ms / 1000);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const mm = h > 0 ? String(m).padStart(2, "0") : String(m);
  return `${h > 0 ? `${h}:` : ""}${mm}:${String(s).padStart(2, "0")}`;
}

// Indexed by SourceKind ordinal (YouTube, SoundCloud, Spotify, AppleMusic, DirectUrl, Attachment).
export const sourceLabel = ["YouTube", "SoundCloud", "Spotify", "Apple Music", "リンク", "ファイル"];

export function sourceName(source: number): string {
  return sourceLabel[source] ?? "";
}

export function guildIconUrl(id: string, icon: string | null): string | null {
  return icon ? `https://cdn.discordapp.com/icons/${id}/${icon}.png?size=128` : null;
}

export function userAvatarUrl(id: string, avatar: string | null): string | null {
  return avatar ? `https://cdn.discordapp.com/avatars/${id}/${avatar}.png?size=64` : null;
}

// A playable track (queue item or history entry) -> the shape the playlists API stores.
export function toPlaylistTrack(t: QueueItem): PlaylistTrack {
  return {
    sourceUrl: t.sourceUrl,
    title: t.title,
    artist: t.artist,
    thumbnailUrl: t.thumbnailUrl,
    source: t.source,
    durationMs: t.durationMs > 0 ? t.durationMs : null,
  };
}
