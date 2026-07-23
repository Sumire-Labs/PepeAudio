// SPDX-License-Identifier: Apache-2.0
// Mirrors the backend PlayerSnapshotDto (SignalR / REST, camelCase) and the playlist DTOs.

export interface Me {
  id: string;
  username: string;
  avatar: string | null;
}

export type PlayerStatus = "idle" | "playing" | "paused";

export interface Guild {
  id: string;
  name: string;
  icon: string | null;
  owner: boolean;
  status: PlayerStatus;
  currentTitle: string | null;
}
export type LoopMode = "off" | "track" | "queue";

// SourceKind ordinal — index into sourceLabel (see lib/format).
export type SourceKind = number;

export interface QueueItem {
  id: string;
  title: string;
  artist: string;
  durationMs: number;
  thumbnailUrl: string | null;
  source: SourceKind;
  sourceUrl: string;
  isLive: boolean;
  requestedBy: string;
  requesterName: string | null;
  requesterAvatarUrl: string | null;
}

export interface PlayerSnapshot {
  guildId: string;
  status: PlayerStatus;
  current: QueueItem | null;
  positionMs: number;
  queue: QueueItem[];
  history: QueueItem[];
  loopMode: LoopMode;
  shuffle: boolean;
  autoplay: boolean;
  volume: number;
  auraEnabled: boolean;
  presetName: string;
  presets: string[];
  crossfadeMs: number;
  epoch: number;
  updatedAt: string;
}

// Playlists (REST)
export interface PlaylistTrack {
  sourceUrl: string;
  title: string;
  artist: string;
  thumbnailUrl: string | null;
  source: SourceKind;
  durationMs: number | null;
}

export interface PlaylistSummary {
  id: string;
  name: string;
  trackCount: number;
  updatedAt: number;
}

export interface PlaylistDetail extends PlaylistSummary {
  tracks: PlaylistTrack[];
}

export interface SearchCandidate {
  title: string;
  author: string;
  url: string;
  thumbnailUrl: string;
}

export interface AdminPlayer {
  id: string;
  name: string;
  playing: boolean;
  current: string | null;
  queue: number;
}

export interface AdminOverview {
  botGuilds: number;
  activeVoices: number;
  shards: number;
  players: AdminPlayer[];
}
