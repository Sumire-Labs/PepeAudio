import type {
  MediaProvider,
  PlayerSnapshot,
  PlayerState,
  RepeatMode,
  TrackProvenance
} from "./types";

export interface PublicMediaPageWire {
  readonly provider: MediaProvider;
  readonly url: string;
}

export interface TrackProvenanceWire {
  readonly origin: PublicMediaPageWire | null;
  readonly playback: PublicMediaPageWire;
}

export interface TrackSnapshotWire {
  readonly track_id: string;
  readonly title: string;
  readonly artist?: string | null;
  readonly album?: string | null;
  readonly provenance?: TrackProvenanceWire | null;
  readonly requester_user_id: string | null;
  readonly duration_ms: number | null;
  readonly position_ms: number;
  readonly seekable: boolean;
}

export interface PlayerSnapshotWire {
  readonly guild_id: string;
  readonly voice_channel_id: string | null;
  readonly revision: number;
  readonly state: PlayerState;
  readonly current_track: TrackSnapshotWire | null;
  readonly queued_tracks: number;
  readonly upcoming_tracks: readonly TrackSnapshotWire[];
  readonly has_previous_track: boolean;
  readonly volume: number;
  readonly repeat_mode: RepeatMode;
  readonly shuffle_enabled: boolean;
  readonly hrir_preset: string | null;
  readonly spatial_audio_enabled: boolean;
  readonly observed_at: number;
}

export type PlayerCommand =
  | { readonly type: "play" }
  | { readonly type: "pause" }
  | { readonly type: "stop" }
  | { readonly type: "skip" }
  | { readonly type: "previous" }
  | { readonly type: "remove_queued"; readonly track_id: string }
  | {
      readonly type: "move_queued";
      readonly track_id: string;
      readonly before_track_id: string | null;
    }
  | { readonly type: "seek"; readonly position_ms: number }
  | { readonly type: "set_volume"; readonly volume: number }
  | { readonly type: "set_repeat"; readonly mode: RepeatMode }
  | { readonly type: "set_shuffle"; readonly enabled: boolean }
  | { readonly type: "set_hrir"; readonly preset: string }
  | { readonly type: "set_spatial_audio"; readonly enabled: boolean };

export function toPlayerSnapshot(wire: PlayerSnapshotWire): PlayerSnapshot {
  const observedAt = validObservedAt(wire.observed_at);
  return {
    guildId: wire.guild_id,
    revision: wire.revision,
    state: wire.state,
    voiceConnected: wire.state !== "disconnected" && wire.voice_channel_id !== null,
    voiceChannelName: null,
    track:
      wire.current_track === null
        ? null
        : {
            id: wire.current_track.track_id,
            title: wire.current_track.title,
            artist: wire.current_track.artist ?? null,
            album: wire.current_track.album ?? null,
            provenance: toProvenance(wire.current_track.provenance),
            requestedBy: null,
            durationMs: wire.current_track.duration_ms,
            positionMsAtAnchor: wire.current_track.position_ms,
            anchorUnixMs: observedAt,
            seekable: wire.current_track.seekable,
            artworkUrl: null
          },
    queue: wire.upcoming_tracks.map((track) => ({
      id: track.track_id,
      title: track.title,
      artist: track.artist ?? null,
      provenance: toProvenance(track.provenance),
      requestedBy: null,
      durationMs: track.duration_ms
    })),
    hasPreviousTrack: wire.has_previous_track,
    volumePercent: wire.volume,
    repeatMode: wire.repeat_mode,
    shuffleEnabled: wire.shuffle_enabled,
    hrirPresetId: wire.hrir_preset,
    spatialEnabled: wire.spatial_audio_enabled,
    observedAtUnixMs: observedAt
  };
}

function toProvenance(
  value: TrackProvenanceWire | null | undefined
): TrackProvenance | null {
  if (value === null || value === undefined) return null;
  return {
    origin: value.origin,
    playback: value.playback
  };
}

function validObservedAt(value: number): number {
  const now = Date.now();
  if (!Number.isSafeInteger(value) || value < 0 || value > now + 60_000) {
    return now;
  }
  return value;
}
