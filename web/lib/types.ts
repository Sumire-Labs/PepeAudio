// SPDX-License-Identifier: Apache-2.0
export interface Me {
  id: string;
  username: string;
  avatar: string | null;
}

export interface Guild {
  id: string;
  name: string;
  icon: string | null;
  owner: boolean;
}

export interface TrackInfo {
  title: string;
  artist: string;
  source: number; // SourceKind
  url: string;
  durationMs: number;
  thumbnailUrl: string | null;
  isLive: boolean;
  requestedBy: string;
}

export interface QueueEntry {
  position: number;
  track: TrackInfo;
}

export interface PlayerState {
  guildId: string;
  current: TrackInfo | null;
  positionMs: number;
  isPlaying: boolean;
  volume: number;
  loop: number; // 0 Off, 1 Track, 2 Queue
  shuffle: boolean;
  auraEnabled: boolean;
  presetName: string;
  crossfadeMs: number;
  queue: QueueEntry[];
  epoch: number;
  updatedAt: string;
}

export type Control =
  | "PlayPause" | "Skip" | "Previous" | "Stop"
  | "Loop" | "Shuffle" | "VolumeUp" | "VolumeDown" | "ToggleAura";

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
