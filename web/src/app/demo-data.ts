import type {
  GuildSummary,
  HrirPreset,
  PlayerSnapshot,
  QueueItem,
  TrackView
} from "./types";

export const demoGuilds: readonly GuildSummary[] = [
  {
    id: "120000000000000001",
    name: "Sumire Listening Room",
    initials: "SL",
    iconUrl: null,
    connected: true,
    active: true,
    listenerCount: 8
  },
  {
    id: "120000000000000002",
    name: "Midnight Workshop",
    initials: "MW",
    iconUrl: null,
    connected: true,
    active: false,
    listenerCount: 3
  },
  {
    id: "120000000000000003",
    name: "Archive Annex",
    initials: "AA",
    iconUrl: null,
    connected: false,
    active: false,
    listenerCount: 0
  },
  {
    id: "120000000000000004",
    name: "Late Night Lab",
    initials: "LL",
    iconUrl: null,
    connected: true,
    active: false,
    listenerCount: 5
  }
];

export const demoPresets: readonly HrirPreset[] = [
  {
    id: "studio-neutral",
    name: "Studio Neutral",
    description: "定位を自然に保つ、残響の少ない基準プリセット。",
    source: {
      licenseName: "Demo data",
      sourceUrl: null,
      attribution: "Tight room · balanced front image"
    }
  },
  {
    id: "wide-hall",
    name: "Wide Hall",
    description: "広い音場と長めの残響を加えるデモプリセット。",
    source: {
      licenseName: "Demo data",
      sourceUrl: null,
      attribution: "Broad stage · longer room tail"
    }
  },
  {
    id: "close-field",
    name: "Close Field",
    description: "近い定位と控えめな空間感を重視したデモプリセット。",
    source: {
      licenseName: "Demo data",
      sourceUrl: null,
      attribution: "Intimate stage · low ambience"
    }
  }
];

const demoTrack: TrackView = {
  id: "84cb4cf6-7e0a-4c5e-b44b-cb8d8df5d37d",
  title: "Signals After Rain",
  artist: "Aster Vale",
  album: "Nocturne Transit",
  requestedBy: "s12kuma01",
  durationMs: 256_000,
  positionMsAtAnchor: 104_000,
  anchorUnixMs: Date.now(),
  seekable: true,
  artworkUrl: null
};

const demoQueue: readonly QueueItem[] = [
  {
    id: "queue-1",
    title: "Glass Meridian",
    artist: "Yuna Field",
    requestedBy: "Nox",
    durationMs: 221_000
  },
  {
    id: "queue-2",
    title: "Soft Relay",
    artist: "Cassette Flora",
    requestedBy: "miso",
    durationMs: 194_000
  },
  {
    id: "queue-3",
    title: "Last Tram Home",
    artist: "Kite Assembly",
    requestedBy: "ruru",
    durationMs: 278_000
  }
];

export function createDemoSnapshot(guildId: string): PlayerSnapshot {
  return {
    guildId,
    revision: 12,
    state: "playing",
    voiceConnected: true,
    voiceChannelName: "Listening Lounge",
    track: demoTrack,
    queue: demoQueue,
    hasPreviousTrack: true,
    volumePercent: 75,
    repeatMode: "off",
    shuffleEnabled: false,
    hrirPresetId: "studio-neutral",
    spatialEnabled: true,
    orbitDegrees: 42,
    observedAtUnixMs: Date.now()
  };
}
