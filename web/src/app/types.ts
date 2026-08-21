export type PlayerState =
  | "disconnected"
  | "idle_connected"
  | "loading"
  | "playing"
  | "paused";

export type RepeatMode = "off" | "track" | "queue";

export type MediaProvider =
  | "spotify"
  | "apple_music"
  | "youtube"
  | "soundcloud";

export interface PublicMediaPage {
  readonly provider: MediaProvider;
  readonly url: string;
}

export interface TrackProvenance {
  readonly origin: PublicMediaPage | null;
  readonly playback: PublicMediaPage;
}

export interface GuildSummary {
  readonly id: string;
  readonly name: string;
  readonly initials: string;
  readonly iconUrl: string | null;
  readonly connected: boolean;
  readonly active: boolean;
  readonly listenerCount: number | null;
}

export interface TrackView {
  readonly id: string;
  readonly title: string;
  readonly artist?: string | null;
  readonly album?: string | null;
  readonly provenance?: TrackProvenance | null;
  readonly requestedBy?: string | null;
  readonly durationMs: number | null;
  readonly positionMsAtAnchor: number;
  readonly anchorUnixMs: number;
  readonly seekable: boolean;
  readonly artworkUrl: string | null;
}

export interface QueueItem {
  readonly id: string;
  readonly title: string;
  readonly artist?: string | null;
  readonly provenance?: TrackProvenance | null;
  readonly requestedBy?: string | null;
  readonly durationMs: number | null;
}

export interface HrirPreset {
  readonly id: string;
  readonly name: string;
  readonly description: string | null;
  readonly source: HrirPresetSource;
}

export interface HrirPresetSource {
  readonly licenseName: string | null;
  readonly sourceUrl: string | null;
  readonly attribution: string | null;
}

export type DashboardAccountSource = "discord" | "development" | "demo";

export interface DashboardAccount {
  readonly source: DashboardAccountSource;
  readonly userId: string | null;
  readonly username: string | null;
  readonly displayName: string;
  readonly avatarUrl: string | null;
}

export type HrirCatalogStatus = "loading" | "ready" | "unavailable";

export interface PlayerSnapshot {
  readonly guildId: string;
  readonly revision: number;
  readonly state: PlayerState;
  readonly voiceConnected: boolean;
  readonly voiceChannelName: string | null;
  readonly track: TrackView | null;
  readonly queue: readonly QueueItem[];
  readonly hasPreviousTrack: boolean;
  readonly volumePercent: number;
  readonly repeatMode: RepeatMode;
  readonly shuffleEnabled: boolean;
  readonly hrirPresetId: string | null;
  readonly spatialEnabled: boolean;
  readonly orbitDegrees: number;
  readonly observedAtUnixMs: number;
}

export interface DashboardModel {
  readonly guilds: readonly GuildSummary[];
  readonly selectedGuildId: string;
  readonly snapshot: PlayerSnapshot;
  readonly presets: readonly HrirPreset[];
  readonly hrirCatalogStatus: HrirCatalogStatus;
  readonly connected: boolean;
  readonly commandPending: boolean;
  readonly selectGuild: (guildId: string) => void;
  readonly togglePlayback: () => void;
  readonly skip: () => void;
  readonly previous: () => void;
  readonly stop: () => void;
  readonly removeQueued: (trackId: string) => void;
  readonly moveQueued: (trackId: string, beforeTrackId: string | null) => void;
  readonly toggleShuffle: () => void;
  readonly cycleRepeat: () => void;
  readonly setVolume: (percent: number) => Promise<void> | void;
  readonly setPreset: (presetId: string) => void;
  readonly toggleSpatial: () => void;
  readonly seek: (positionMs: number) => Promise<void> | void;
}

export type DashboardStatus =
  | "connecting"
  | "reconnecting"
  | "ready"
  | "unauthenticated"
  | "unavailable";

export interface DashboardSession {
  readonly status: DashboardStatus;
  readonly model: DashboardModel;
  readonly account: DashboardAccount | null;
  readonly usingDemoData: boolean;
  readonly message: string | null;
  readonly feedback: DashboardFeedback | null;
  readonly retry: () => void;
  readonly login: () => void;
  readonly logout: (() => void) | null;
  readonly loggingOut: boolean;
}

export interface DashboardFeedback {
  readonly id: number;
  readonly message: string;
  readonly type: "error" | "info";
}
