import { describe, expect, it } from "vitest";

import { parsePlayerSnapshotWire } from "./player-sse";
import { toPlayerSnapshot, type PlayerSnapshotWire } from "./wire-types";

describe("toPlayerSnapshot", () => {
  it("preserves the authoritative upcoming queue order", () => {
    const wire: PlayerSnapshotWire = {
      guild_id: "1",
      voice_channel_id: "2",
      revision: 4,
      state: "playing",
      current_track: null,
      queued_tracks: 2,
      upcoming_tracks: [
        track("first", "First"),
        track("second", "Second")
      ],
      has_previous_track: false,
      volume: 75,
      repeat_mode: "off",
      shuffle_enabled: true,
      hrir_preset: null,
      spatial_audio_enabled: false,
      observed_at: 0
    };

    expect(toPlayerSnapshot(wire).queue.map((item) => item.id)).toEqual([
      "first",
      "second"
    ]);
    expect(toPlayerSnapshot(wire).hasPreviousTrack).toBe(false);
  });

  it("anchors playing progress to the server observation time", () => {
    const observedAt = Date.now() - 12_000;
    const wire: PlayerSnapshotWire = {
      guild_id: "1",
      voice_channel_id: "2",
      revision: 5,
      state: "playing",
      current_track: { ...track("current", "Current"), position_ms: 4_000 },
      queued_tracks: 0,
      upcoming_tracks: [],
      has_previous_track: false,
      volume: 75,
      repeat_mode: "off",
      shuffle_enabled: false,
      hrir_preset: null,
      spatial_audio_enabled: false,
      observed_at: observedAt
    };

    const snapshot = toPlayerSnapshot(wire);
    expect(snapshot.observedAtUnixMs).toBe(observedAt);
    expect(snapshot.track?.anchorUnixMs).toBe(observedAt);
  });

  it("does not invent track labels or expose a voice-channel snowflake as a name", () => {
    const wire: PlayerSnapshotWire = {
      guild_id: "1",
      voice_channel_id: "18446744073709551615",
      revision: 6,
      state: "playing",
      current_track: track("current", "Current"),
      queued_tracks: 1,
      upcoming_tracks: [track("next", "Next")],
      has_previous_track: false,
      volume: 75,
      repeat_mode: "off",
      shuffle_enabled: false,
      hrir_preset: null,
      spatial_audio_enabled: false,
      observed_at: 0
    };

    const snapshot = toPlayerSnapshot(wire);
    expect(snapshot.voiceConnected).toBe(true);
    expect(snapshot.voiceChannelName).toBeNull();
    expect(snapshot.track).toMatchObject({
      artist: null,
      album: null,
      requestedBy: null
    });
    expect(snapshot.queue[0]).toMatchObject({ artist: null, requestedBy: null });
  });

  it("maps validated display metadata and provenance", () => {
    const provenance = {
      origin: {
        provider: "spotify" as const,
        url: "https://open.spotify.com/track/4uLU6hMCjMI75M1A2tKUQC"
      },
      playback: {
        provider: "youtube" as const,
        url: "https://www.youtube.com/watch?v=dQw4w9WgXcQ"
      }
    };
    const wire: PlayerSnapshotWire = {
      guild_id: "1",
      voice_channel_id: "2",
      revision: 7,
      state: "playing",
      current_track: {
        ...track("00000000-0000-0000-0000-000000000001", "Never Gonna Give You Up"),
        artist: "Rick Astley",
        album: "Whenever You Need Somebody",
        provenance
      },
      queued_tracks: 1,
      upcoming_tracks: [{
        ...track("00000000-0000-0000-0000-000000000002", "Together Forever"),
        artist: "Rick Astley",
        provenance
      }],
      has_previous_track: false,
      volume: 75,
      repeat_mode: "off",
      shuffle_enabled: false,
      hrir_preset: null,
      spatial_audio_enabled: false,
      observed_at: 0
    };

    const snapshot = toPlayerSnapshot(parsePlayerSnapshotWire(wire, "1"));

    expect(snapshot.track).toMatchObject({
      artist: "Rick Astley",
      album: "Whenever You Need Somebody",
      provenance
    });
    expect(snapshot.queue[0]).toMatchObject({ artist: "Rick Astley", provenance });
  });

  it("rejects a signed stream locator presented as public provenance", () => {
    const wire: PlayerSnapshotWire = {
      guild_id: "1",
      voice_channel_id: "2",
      revision: 8,
      state: "playing",
      current_track: {
        ...track("00000000-0000-0000-0000-000000000003", "Current"),
        provenance: {
          origin: null,
          playback: {
            provider: "youtube",
            url: "https://rr1---sn.example.googlevideo.com/videoplayback?sig=secret"
          }
        }
      },
      queued_tracks: 0,
      upcoming_tracks: [],
      has_previous_track: false,
      volume: 75,
      repeat_mode: "off",
      shuffle_enabled: false,
      hrir_preset: null,
      spatial_audio_enabled: false,
      observed_at: 0
    };

    expect(() => parsePlayerSnapshotWire(wire, "1")).toThrow("invalid");
  });

  it("rejects a noncanonical Apple Music origin path", () => {
    const wire: PlayerSnapshotWire = {
      guild_id: "1",
      voice_channel_id: "2",
      revision: 9,
      state: "playing",
      current_track: {
        ...track("00000000-0000-0000-0000-000000000004", "Current"),
        provenance: {
          origin: {
            provider: "apple_music",
            url: "https://music.apple.com/jp/album/extra/example/1440833098?i=1440833542"
          },
          playback: {
            provider: "youtube",
            url: "https://www.youtube.com/watch?v=dQw4w9WgXcQ"
          }
        }
      },
      queued_tracks: 0,
      upcoming_tracks: [],
      has_previous_track: false,
      volume: 75,
      repeat_mode: "off",
      shuffle_enabled: false,
      hrir_preset: null,
      spatial_audio_enabled: false,
      observed_at: 0
    };

    expect(() => parsePlayerSnapshotWire(wire, "1")).toThrow("invalid");
  });
});

function track(id: string, title: string) {
  return {
    track_id: id,
    title,
    requester_user_id: "3",
    duration_ms: 1_000,
    position_ms: 0,
    seekable: true
  };
}
