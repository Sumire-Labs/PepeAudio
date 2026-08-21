import { discordGuildIconUrl, type AuthGuild } from "./auth-wire";
import type {
  DashboardModel,
  GuildSummary,
  HrirCatalogStatus,
  HrirPreset,
  PlayerSnapshot,
  RepeatMode
} from "./types";
import type { PlayerCommand } from "./wire-types";

interface LiveDashboardModelInput {
  readonly guilds: readonly AuthGuild[];
  readonly selectedGuildId: string;
  readonly snapshot: PlayerSnapshot | null;
  readonly presets: readonly HrirPreset[];
  readonly catalogStatus: HrirCatalogStatus;
  readonly commandPending: boolean;
  readonly run: (command: PlayerCommand) => Promise<void>;
  readonly selectGuild: (guildId: string) => void;
}

export function buildLiveDashboardModel({
  guilds,
  selectedGuildId,
  snapshot,
  presets,
  catalogStatus,
  commandPending,
  run,
  selectGuild
}: LiveDashboardModelInput): DashboardModel {
  const current = snapshot ?? disconnectedSnapshot(selectedGuildId);
  return {
    guilds: guilds.map((guild) => toGuildSummary(guild, selectedGuildId, snapshot)),
    selectedGuildId,
    snapshot: current,
    presets,
    hrirCatalogStatus: catalogStatus,
    connected: snapshot !== null,
    commandPending,
    selectGuild,
    togglePlayback: () => void run({ type: current.state === "playing" ? "pause" : "play" }),
    skip: () => void run({ type: "skip" }),
    previous: () => void run({ type: "previous" }),
    stop: () => void run({ type: "stop" }),
    removeQueued: (trackId) => void run({ type: "remove_queued", track_id: trackId }),
    moveQueued: (trackId, beforeTrackId) => void run({
      type: "move_queued",
      track_id: trackId,
      before_track_id: beforeTrackId
    }),
    toggleShuffle: () => void run({ type: "set_shuffle", enabled: !current.shuffleEnabled }),
    cycleRepeat: () => void run({ type: "set_repeat", mode: nextRepeat(current.repeatMode) }),
    setVolume: (volume) => run({ type: "set_volume", volume: Math.min(100, volume) }),
    setPreset: (preset) => void run({ type: "set_hrir", preset }),
    toggleSpatial: () => void run({ type: "set_spatial_audio", enabled: !current.spatialEnabled }),
    seek: (positionMs) => run({ type: "seek", position_ms: positionMs })
  };
}

export function selectInitialGuild(
  current: string,
  guilds: readonly AuthGuild[]
): string {
  if (guilds.some((guild) => guild.id === current && guild.botPresent)) return current;
  return guilds.find((guild) => guild.botPresent)?.id ?? "";
}

function toGuildSummary(
  guild: AuthGuild,
  selectedId: string,
  snapshot: PlayerSnapshot | null
): GuildSummary {
  const connected =
    guild.id === selectedId && snapshot !== null && snapshot.state !== "disconnected";
  return {
    id: guild.id,
    name: guild.name,
    initials: initials(guild.name),
    iconUrl: discordGuildIconUrl(guild.id, guild.icon),
    connected,
    active: guild.botPresent,
    listenerCount: null
  };
}

function initials(name: string): string {
  const words = name.trim().split(/\s+/u).filter(Boolean);
  return (words.length > 1 ? `${words[0]?.[0] ?? ""}${words[1]?.[0] ?? ""}` : name.slice(0, 2))
    .toLocaleUpperCase();
}

function disconnectedSnapshot(guildId: string): PlayerSnapshot {
  return {
    guildId,
    revision: 0,
    state: "disconnected",
    voiceConnected: false,
    voiceChannelName: null,
    track: null,
    queue: [],
    hasPreviousTrack: false,
    volumePercent: 10,
    repeatMode: "off",
    shuffleEnabled: false,
    hrirPresetId: null,
    spatialEnabled: false,
    observedAtUnixMs: Date.now()
  };
}

function nextRepeat(mode: RepeatMode): RepeatMode {
  return mode === "off" ? "track" : mode === "track" ? "queue" : "off";
}
